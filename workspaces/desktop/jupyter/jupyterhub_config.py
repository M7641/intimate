# JupyterHub configuration.
#
# We run JupyterHub 5.3+ specifically for the JUPYTERHUB_XSRF_ANONYMOUS_IP_CIDRS escape
# hatch (set in the Dockerfile ENV) that fixes the login-page XSRF 403 behind Nimbus's Istio
# proxy. JupyterHub 5.0 also made Authenticator.allow_all default to False — so an
# authenticated user would be rejected after a correct password. On Nimbus the single OS user
# is created by the platform's postStart hook and authenticated via PAM, so we allow any
# authenticated user.
c = get_config()  # noqa: F821
c.Authenticator.allow_all = True
