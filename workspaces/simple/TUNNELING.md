# Tunneling with VS Code

This README provides instructions for setting up and managing remote tunnels using this image, making it easier for you to access your workspace remotely within VS Code.

## What is VS Code Tunneling ?

VS Code Tunneling is a feature that allows users to connect to a remote machine, such as a desktop PC or virtual machine, via a secure tunnel. This feature securely transmits data from one network to another, allowing users to develop against any machine of their choosing from a VS Code desktop or web client without the need for SSH or HTTPS setup. For more information, please refer (this)[https://code.visualstudio.com/docs/remote/tunnels] link.

## Initializing a Remote Tunnel

To create a new remote tunnel for your workspace, you can use the provided bash script:

```bash
start-tunnel
```

Or for Cursor, run the following command

```bash
start-tunnel cursor
```

Running this script initializes and starts a new tunnel in the background, which can be accessed remotely. To ensure its functionality, a brief authentication step using GitHub is required.

After executing the above command, you will see the following message in the console:

```
To grant access to the server, please log into https://github.com/login/device and use code <access-code>
```

Follow the link provided and enter the `<access-code>` displayed in the console.

That's it! Your new, secure tunnel should now be active and accessible at the following URL: `https://vscode.dev/tunnel/workspace-<workspace-id>-0`

## Managing Tunnels

### Restarting a Tunnel

If you need to restart your tunnel for smooth operation, you can use the following command:

```bash
restart-tunnel
```

Or for Cursor, run the following command

```bash
restart-tunnel cursor
```

### Stopping a Tunnel

To stop an active tunnel, use the following command:

```bash
stop-tunnel
```

Or for Cursor, run the following command

```bash
stop-tunnel cursor
```

### Starting a Tunnel

To start a previously stopped tunnel, use this command:

```bash
start-tunnel
```

Or for Cursor, run the following command

```bash
start-tunnel cursor
```

### Verifying the status of tunnel

To check the status of the tunnel, use this command:

```bash
tunnel-status
```

Or for Cursor, run the following command

```bash
tunnel-status cursor
```

## Additional Notes

To view the logs of any running or closed tunnel, you can refer to the `output.log` file located at `/vscode-tunnel/logs/output.log`. Similarly, any errors related to running or closed tunnels can be found in the `/vscode-tunnel/logs/error.log` file.
