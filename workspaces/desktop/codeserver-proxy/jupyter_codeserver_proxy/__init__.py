import os
import subprocess

bash_command = "code-server --install-extension cweijan.vscode-database-client2@6.6.3"


def setup_codeserver():
    _install_vscode_extension()
    return {
        "command": [
            "code-server",
            "--auth",
            "none",
            "--disable-telemetry",
            "--port={port}",
        ],
        "launcher_entry": {
            "title": "VS Code",
            "icon_path": os.path.join(
                os.path.dirname(os.path.abspath(__file__)), "icons", "vs_code_icon.svg"
            ),
        },
    }


def _install_vscode_extension():
    # Run the Bash command and capture the output
    try:
        result = subprocess.check_output(bash_command, shell=True, text=True)
        print("Bash command output:")
        print(result)
    except subprocess.CalledProcessError as e:
        print("Error running the Bash command:")
        print(e)
