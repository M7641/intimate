

APP_NAME="$1"

if [ "${APP_NAME,,}" == "cursor" ]; then
  cursor tunnel status
else
  code tunnel status
fi
