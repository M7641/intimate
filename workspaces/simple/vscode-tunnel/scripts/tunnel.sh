APP_NAME="$1"

if [ "${APP_NAME,,}" == "cursor" ]; then
  cursor tunnel --verbose
else
  code tunnel
fi
