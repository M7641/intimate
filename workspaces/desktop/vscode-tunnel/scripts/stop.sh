APP_NAME="$1"

if [ "${APP_NAME,,}" == "cursor" ]; then
  cursor tunnel kill
else
  code tunnel kill
fi
