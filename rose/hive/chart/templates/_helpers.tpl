{{/*
Standard name + label helpers. Keeping these in one place means every resource
carries the SAME selector labels — and the Service finding its pods depends
entirely on those labels matching, so a copy-paste drift here is a silent outage.
*/}}

{{- define "hive.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "hive.fullname" -}}
{{- printf "%s-%s" .Release.Name (include "hive.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{/* Labels applied to every object — for humans and tooling. */}}
{{- define "hive.labels" -}}
app.kubernetes.io/name: {{ include "hive.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version }}
{{- end -}}

{{/*
Selector labels — the STABLE subset. A Deployment's selector is immutable, and
the Service routes on exactly this set, so it must never include volatile fields
like the version.
*/}}
{{- define "hive.selectorLabels" -}}
app.kubernetes.io/name: {{ include "hive.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}
