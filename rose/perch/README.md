# perch

> Héberger un serveur Axum (Rust) sur AWS, **~$0/mois idle**, avec le confort
> production : TLS managé, edge cache, observabilité, déploiements immutables,
> IAM least-privilege, alarmes et rollback en une commande.

Pulumi (Python) en provisioning ; Docker pour l'image ; CloudFront → Lambda
Function URL → Lambda container ARM64 → Lambda Web Adapter → Axum.

## Architecture

```
        Internet
           │
           ▼
    ┌──────────────┐   TLS gratuit, edge cache (1 TB/mo free year 1)
    │  CloudFront  │   PriceClass_100 = US/CA/EU edges seulement
    └──────┬───────┘
           │ https-only, Host stripé
           ▼
    ┌──────────────────────┐  $0 + prix Lambda standard (pas d'API Gateway)
    │ Lambda Function URL  │
    └──────┬───────────────┘
           │ alias "live" → version épinglée
           ▼
    ┌──────────────────────┐  ARM64 Graviton ≈ 20 % moins cher que x86
    │ Lambda (container)   │  scales-to-zero, free tier 1M req/mo
    └──────┬───────────────┘
           │ AWS_LWA_INVOKE_MODE=buffered
           ▼
    ┌──────────────────────┐  intercepte l'invocation, proxie localhost:8080
    │  Lambda Web Adapter  │
    └──────┬───────────────┘
           ▼
    ┌──────────────────────┐  graceful shutdown sur SIGTERM, JSON logs
    │   Axum binary        │
    └──────────────────────┘

  CloudWatch Logs (rétention 14 j) ──┐
  X-Ray traces (free tier)           ├─► observabilité
  SNS topic ◄── 3 MetricAlarms (5xx, throttles, p99 duration)
  SSM Parameter Store /perch/{stage}/* ──► config app
```

## Pourquoi ces choix

| Choix                      | Alternative écartée           | Raison                                                                                                                             |
| -------------------------- | ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| **Lambda container**       | Fargate Spot, App Runner, EC2 | Seul AWS-native qui scale-to-zero ($0 idle).                                                                                       |
| **Function URL**           | API Gateway HTTP API          | API GW = $1/M req fixes ; Function URL = $0 marginal.                                                                              |
| **CloudFront devant**      | Function URL nu               | TLS sur domaine custom, masquage hostname Lambda, edge cache.                                                                      |
| **ARM64 (Graviton)**       | x86_64                        | ~20 % moins cher, Rust se cross-compile bien.                                                                                      |
| **Container image**        | ZIP + custom runtime          | Lambda Web Adapter dispense d'écrire un handler `lambda_runtime` ; le code Axum reste portable (tourne aussi en local sans modif). |
| **Alias `live` + version** | $LATEST                       | Rollback en `aws lambda update-alias`, blue/green possible.                                                                        |
| **Pulumi Python**          | Terraform, CDK                | Cohérent avec `galicia/`, awsx fait le ECR-build-and-push proprement.                                                              |

## Coût (eu-west-3, hors traffic significatif)

| Service                       | Free tier                              | Au-delà                           | À 100 k req/mo              |
| ----------------------------- | -------------------------------------- | --------------------------------- | --------------------------- |
| Lambda (ARM, 512 MB, ~200 ms) | 1 M req + 400 k GB-s / mois            | $0.20/M req + $0.0000133/GB-s     | ~$0.02 + ~$0.13 = **$0.15** |
| Lambda Function URL           | inclus                                 | inclus                            | $0                          |
| CloudFront                    | 1 TB/mo _year 1_, puis 50 GB/mo always | $0.085/GB                         | ~$0 si réponses petites     |
| CloudWatch Logs               | 5 GB ingest + 5 GB stored              | $0.50/GB ingest + $0.03/GB stored | <$0.05                      |
| ECR                           | 500 MB / mois                          | $0.10/GB                          | ~$0.01                      |
| SSM Parameter Store           | standard params gratuits               | —                                 | $0                          |
| X-Ray                         | 100 k traces / mo                      | $5/M traces                       | $0                          |
| SNS                           | 1 k notifications / mo                 | $0.50/M                           | $0                          |
| **Total idle**                |                                        |                                   | **~$0**                     |
| **Total 100 k req/mo**        |                                        |                                   | **<$1**                     |

Domaine custom (Route 53) ajoute $0.50/mo si vous voulez `perch.votredomaine.io`.

## Prérequis

- Pulumi CLI installé (`brew install pulumi`)
- AWS credentials configurées (`aws configure` ou SSO)
- Docker buildx (Docker Desktop ou colima) — `docker build --platform=linux/arm64` doit fonctionner
- Python ≥ 3.10
- `just` (optionnel mais conseillé) : `brew install just`

## Quickstart

```bash
cd rose/perch
python -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
pulumi stack init dev          # première fois seulement
pulumi config set aws:region eu-west-3
just up                         # = pulumi up
just smoke                      # curl GET / /healthz /env
just logs                       # tail CloudWatch
```

Pour brancher les alarmes par mail :

```bash
pulumi config set perch:alarmEmail vous@example.com
pulumi up
# Confirmez le mail SNS reçu.
```

## Déploiements immutables et rollback

Chaque `pulumi up` qui modifie l'app :

1. Rebuilde l'image Docker.
2. Push l'image avec un nouveau digest dans ECR.
3. Met à jour le code de la Lambda, ce qui **publie une nouvelle version**
   (parce que `publish=True`).
4. Met à jour l'alias `live` pour pointer vers cette nouvelle version.
5. CloudFront pointe sur `<function-name>:live`, donc le switch est atomique.

Rollback :

```bash
aws lambda list-versions-by-function --function-name perch-dev
just rollback 4   # bascule live → version 4, pas de redeploy
```

## Ce qu'il faudrait ajouter pour aller plus loin

- **Domaine custom + ACM cert** dans us-east-1 (CloudFront exige la région) :
  ~20 lignes Pulumi en plus, $0.50/mo Route 53.
- **WAF** sur la distribution CloudFront : ~$5/mo + $1/M requêtes inspectées.
  Vaut le coup dès qu'il y a quelque chose à protéger.
- **Provisioned concurrency** sur l'alias : élimine les cold starts pour les
  endpoints critiques, ~$10/mo pour 1 instance pré-warmed 24/7.
- **CodeDeploy** pour faire du canary 10 %/90 % automatique sur 5 min :
  c'est ce que `aws lambda update-alias --routing-config` permet déjà
  manuellement, CodeDeploy l'automatise + auto-rollback sur alarme.
- **Secrets Manager** au lieu de SSM pour les secrets rotatifs (DB password,
  API keys) : $0.40/mo par secret.

## Sortie de `pulumi up` (anatomie)

```
Outputs:
    alarm_topic_arn   : "arn:aws:sns:eu-west-3:…:perch-dev-alarms"
    cloudfront_url    : "https://d1xxxxx.cloudfront.net"
    function_name     : "perch-dev"
    function_url      : "https://xxxxx.lambda-url.eu-west-3.on.aws/"
    function_version  : "3"
    log_group         : "/aws/lambda/perch-dev"
    ssm_greeting_param: "/perch/dev/greeting"
```

Tester directement la Function URL bypasse CloudFront — utile pour isoler
si un problème vient de la Lambda ou de CloudFront.
