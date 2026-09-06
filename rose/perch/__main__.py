"""perch — Axum HTTP server sur AWS Lambda, ~$0/mois idle, production-grade.

Architecture (descendante) :

    Internet
       │
       ▼
    CloudFront  ────────────────  TLS, edge cache 1 TB/mo free year-1
       │                          (origine HTTPS only, pas de Host forward)
       ▼
    Lambda Function URL  ───────  pas d'API Gateway = pas de coût/req fixe
       │
       ▼
    Lambda (container, ARM64)  ─  scales-to-zero, Graviton 20 % moins cher
       │  alias "live" → version épinglée
       ▼
    Lambda Web Adapter (ext)  ──  proxie l'invocation Lambda vers localhost:8080
       │
       ▼
    Axum binary (Tokio)  ───────  graceful shutdown sur SIGTERM, JSON logs

Observabilité :
  • CloudWatch Logs (groupe pré-créé avec rétention, sinon Lambda crée infinite)
  • Alarmes 5xx / Throttles / Duration p99 → SNS topic (mail si configuré)
  • X-Ray actif (free tier 100 k traces/mo)

Config :
  • SSM Parameter Store (chiffré KMS gratuit)
  • IAM role Lambda lit /perch/{stage}/* seulement (least privilege)

Tout est tagué {app=perch, stage=<stack>} pour cost allocation.
"""

import json
from pathlib import Path

import pulumi
import pulumi_aws as aws
import pulumi_awsx as awsx

# ───────────────────────── config ─────────────────────────
cfg = pulumi.Config()
stage = pulumi.get_stack()
alarm_email = cfg.get("alarmEmail")
log_retention_days = cfg.get_int("logRetentionDays") or 14
lambda_memory_mb = cfg.get_int("lambdaMemoryMb") or 512
lambda_timeout_s = cfg.get_int("lambdaTimeoutSeconds") or 15

app_dir = str((Path(__file__).parent / "app").resolve())
common_tags = {"app": "perch", "stage": stage, "managed_by": "pulumi"}

# ───────────────────────── 1. image OCI ─────────────────────────
# Le repo ECR a une lifecycle policy : on ne garde que les 5 dernières images.
# Sinon chaque déploiement empile ~80 MB et la facture ECR dérive en silence.
repo = awsx.ecr.Repository(
    "perch",
    name=f"perch-{stage}",
    force_delete=True,
    lifecycle_policy=awsx.ecr.LifecyclePolicyArgs(
        rules=[
            awsx.ecr.LifecyclePolicyRuleArgs(
                description="keep last 5 images",
                maximum_number_of_images=5,
                tag_status="any",
            )
        ]
    ),
    tags=common_tags,
)

# `awsx.ecr.Image` exécute `docker build` localement puis push. L'URI retourné
# inclut le digest, donc le diff Pulumi détecte un changement de contenu même
# si le tag est le même — c'est ce qui rend les déploiements immutables.
image = awsx.ecr.Image(
    "perch",
    repository_url=repo.url,
    context=app_dir,
    dockerfile=f"{app_dir}/Dockerfile",
    platform="linux/arm64",
)

# ───────────────────────── 2. IAM role (least privilege) ─────────────────────────
assume_lambda = aws.iam.get_policy_document(
    statements=[
        aws.iam.GetPolicyDocumentStatementArgs(
            actions=["sts:AssumeRole"],
            principals=[
                aws.iam.GetPolicyDocumentStatementPrincipalArgs(
                    type="Service", identifiers=["lambda.amazonaws.com"]
                )
            ],
        )
    ]
)
role = aws.iam.Role(
    "perch-exec",
    name=f"perch-{stage}-exec",
    assume_role_policy=assume_lambda.json,
    tags=common_tags,
)

# Policy AWS-managed pour les permissions CloudWatch Logs minimales.
aws.iam.RolePolicyAttachment(
    "perch-basic-logs",
    role=role.name,
    policy_arn="arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole",
)
# X-Ray write permissions (les traces ne partent nulle part sans ça).
aws.iam.RolePolicyAttachment(
    "perch-xray",
    role=role.name,
    policy_arn="arn:aws:iam::aws:policy/AWSXRayDaemonWriteAccess",
)

# ───────────────────────── 3. SSM config ─────────────────────────
greeting = aws.ssm.Parameter(
    "perch-greeting",
    name=f"/perch/{stage}/greeting",
    type="String",
    value="hello from perch (configured via SSM)",
    tags=common_tags,
)

# La policy SSM est scopée au préfixe /perch/{stage}/ uniquement. Si l'app
# essaie de lire un autre paramètre AWS, IAM la bloque — c'est la séparation
# qu'on perd toujours en collant `*` partout.
ssm_read = aws.iam.Policy(
    "perch-ssm-read",
    name=f"perch-{stage}-ssm-read",
    policy=aws.get_region_output().name.apply(
        lambda region: json.dumps(
            {
                "Version": "2012-10-17",
                "Statement": [
                    {
                        "Effect": "Allow",
                        "Action": [
                            "ssm:GetParameter",
                            "ssm:GetParameters",
                            "ssm:GetParametersByPath",
                        ],
                        "Resource": f"arn:aws:ssm:{region}:*:parameter/perch/{stage}/*",
                    }
                ],
            }
        )
    ),
)
aws.iam.RolePolicyAttachment("perch-ssm-read", role=role.name, policy_arn=ssm_read.arn)

# ───────────────────────── 4. log group avec rétention ─────────────────────────
# IMPORTANT : si on laisse Lambda auto-créer le log group, la rétention est
# "never expire" par défaut. Sur des fonctions chatty c'est la source classique
# de factures CloudWatch qui dérivent à 6 mois.
log_group = aws.cloudwatch.LogGroup(
    "perch-logs",
    name=f"/aws/lambda/perch-{stage}",
    retention_in_days=log_retention_days,
    tags=common_tags,
)

# ───────────────────────── 5. Lambda function ─────────────────────────
fn = aws.lambda_.Function(
    "perch",
    name=f"perch-{stage}",
    package_type="Image",
    image_uri=image.image_uri,
    role=role.arn,
    architectures=["arm64"],  # Graviton : ~20 % moins cher, perf équivalente.
    memory_size=lambda_memory_mb,
    timeout=lambda_timeout_s,
    environment=aws.lambda_.FunctionEnvironmentArgs(
        variables={
            "PERCH_STAGE": stage,
            "RUST_LOG": "info,tower_http=info",
            "AWS_LWA_INVOKE_MODE": "buffered",
        },
    ),
    tracing_config=aws.lambda_.FunctionTracingConfigArgs(mode="Active"),
    publish=True,  # nécessaire pour épingler une version sur l'alias.
    tags=common_tags,
    opts=pulumi.ResourceOptions(depends_on=[log_group]),
)

# Alias "live" : on pointe la Function URL ici, pas sur $LATEST. Ça permet :
#   • blue/green via `aws lambda update-alias --routing-config`
#   • rollback en une commande sans redéployer l'image
#   • observabilité par alias (métriques séparées par version)
alias = aws.lambda_.Alias(
    "perch-live",
    name="live",
    function_name=fn.name,
    function_version=fn.version,
)

fn_url = aws.lambda_.FunctionUrl(
    "perch",
    function_name=fn.name,
    qualifier=alias.name,
    authorization_type="NONE",  # auth lives au niveau CloudFront/WAF si besoin
    invoke_mode="BUFFERED",
    cors=aws.lambda_.FunctionUrlCorsArgs(
        allow_origins=["*"],
        allow_methods=["*"],
        allow_headers=["*"],
        max_age=86400,
    ),
)

# ───────────────────────── 6. CloudFront ─────────────────────────
# Cache policies AWS-managed : pas besoin de les recréer (gratuit, partagées).
cache_disabled = aws.cloudfront.get_cache_policy(name="Managed-CachingDisabled")
origin_request_all = aws.cloudfront.get_origin_request_policy(
    name="Managed-AllViewerExceptHostHeader",
)

origin_host = fn_url.function_url.apply(
    lambda u: u.replace("https://", "").rstrip("/")
)

cf = aws.cloudfront.Distribution(
    "perch",
    enabled=True,
    is_ipv6_enabled=True,
    http_version="http2and3",
    # PriceClass_100 = edges US / CA / EU seulement. Asia/SA coûtent plus.
    # Pour une app mondiale, passer en PriceClass_All ; pour ~$0/mo on reste ici.
    price_class="PriceClass_100",
    origins=[
        aws.cloudfront.DistributionOriginArgs(
            origin_id="perch-lambda",
            domain_name=origin_host,
            custom_origin_config=aws.cloudfront.DistributionOriginCustomOriginConfigArgs(
                http_port=80,
                https_port=443,
                origin_protocol_policy="https-only",
                origin_ssl_protocols=["TLSv1.2"],
            ),
        )
    ],
    default_cache_behavior=aws.cloudfront.DistributionDefaultCacheBehaviorArgs(
        target_origin_id="perch-lambda",
        viewer_protocol_policy="redirect-to-https",
        allowed_methods=["GET", "HEAD", "OPTIONS", "PUT", "POST", "PATCH", "DELETE"],
        cached_methods=["GET", "HEAD"],
        cache_policy_id=cache_disabled.id,
        # AllViewerExceptHostHeader : Function URL rejette les Host non-AWS,
        # CloudFront doit donc *retirer* le Host avant de forward.
        origin_request_policy_id=origin_request_all.id,
        compress=True,
    ),
    viewer_certificate=aws.cloudfront.DistributionViewerCertificateArgs(
        cloudfront_default_certificate=True,
    ),
    restrictions=aws.cloudfront.DistributionRestrictionsArgs(
        geo_restriction=aws.cloudfront.DistributionRestrictionsGeoRestrictionArgs(
            restriction_type="none",
        ),
    ),
    tags=common_tags,
)

# ───────────────────────── 7. alarmes ─────────────────────────
alarm_topic = aws.sns.Topic(
    "perch-alarms",
    name=f"perch-{stage}-alarms",
    tags=common_tags,
)
if alarm_email:
    aws.sns.TopicSubscription(
        "perch-alarms-email",
        topic=alarm_topic.arn,
        protocol="email",
        endpoint=alarm_email,
    )

# Le triplet d'alarmes que toute Lambda en prod mérite :
#   1. Errors  → bug applicatif ou crash du runtime
#   2. Throttles → on tape la concurrent-execution cap (defaut 1000/region)
#   3. Duration p99 → on s'approche du timeout, alerte avant que ça casse
aws.cloudwatch.MetricAlarm(
    "perch-5xx",
    name=f"perch-{stage}-5xx",
    comparison_operator="GreaterThanThreshold",
    evaluation_periods=2,
    metric_name="Errors",
    namespace="AWS/Lambda",
    period=60,
    statistic="Sum",
    threshold=5,
    dimensions={"FunctionName": fn.name},
    alarm_actions=[alarm_topic.arn],
    treat_missing_data="notBreaching",
    tags=common_tags,
)
aws.cloudwatch.MetricAlarm(
    "perch-throttle",
    name=f"perch-{stage}-throttle",
    comparison_operator="GreaterThanThreshold",
    evaluation_periods=1,
    metric_name="Throttles",
    namespace="AWS/Lambda",
    period=60,
    statistic="Sum",
    threshold=0,
    dimensions={"FunctionName": fn.name},
    alarm_actions=[alarm_topic.arn],
    treat_missing_data="notBreaching",
    tags=common_tags,
)
aws.cloudwatch.MetricAlarm(
    "perch-p99-duration",
    name=f"perch-{stage}-p99-duration",
    comparison_operator="GreaterThanThreshold",
    evaluation_periods=3,
    metric_name="Duration",
    namespace="AWS/Lambda",
    period=300,
    extended_statistic="p99",
    # 2 s = 13 % du timeout par défaut. Alerte avant que le timeout coupe.
    threshold=2000,
    dimensions={"FunctionName": fn.name},
    alarm_actions=[alarm_topic.arn],
    treat_missing_data="notBreaching",
    tags=common_tags,
)

# ───────────────────────── outputs ─────────────────────────
pulumi.export("function_name", fn.name)
pulumi.export("function_version", fn.version)
pulumi.export("function_url", fn_url.function_url)
pulumi.export("cloudfront_url", cf.domain_name.apply(lambda d: f"https://{d}"))
pulumi.export("log_group", log_group.name)
pulumi.export("alarm_topic_arn", alarm_topic.arn)
pulumi.export("ssm_greeting_param", greeting.name)
