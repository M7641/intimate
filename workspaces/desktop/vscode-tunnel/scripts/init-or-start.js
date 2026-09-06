/**
 * The script aims to setup a vscode tunnel for the workspace. It does by starting a tunnel in the background.
 * Usage - node /vscode-tunnel/scripts setup.js
 */
const { spawn } = require('child_process');
const fs = require('fs');

const outputLogFilePath = '/vscode-tunnel/logs/output.log';
const errorLogFilePath = '/vscode-tunnel/logs/error.log';
const scriptPath = '/vscode-tunnel/scripts/tunnel.sh';

function sleep(milliseconds) {
  const start = new Date().getTime();
  while (new Date().getTime() - start < milliseconds);
}

function waitForDeviceVerification() {
  while (true) {
    console.log('Waiting for device verification...');
    try {
      const data = fs.readFileSync(outputLogFilePath, 'utf8');
      const index = data.lastIndexOf('Open this link in your browser');
      if (index >= 0) {
        console.log(data.substring(index));
        break;
      }
      // check for errors if any
      const errors = fs.readFileSync(errorLogFilePath, 'utf8');
      if (errors.length) {
        console.error('There was an error creating the tunnel.');
        console.error(errors);
        break;
      }
      sleep(4000);
    } catch (error) {
      if (error.code === 'ENOENT') {
        console.log(`File '${fileName}' not found.`);
      } else {
        console.error(`An error occurred: ${error}`);
      }
      break;
    }
  }
}

function printOutput() {
  // Read the file and print its contents
  try {
    const data = fs.readFileSync(outputLogFilePath, 'utf8');
    const index = data.lastIndexOf('Visual Studio Code Server');
    if (index >= 0) {
      console.log(data.substring(index));
    } else {
      console.log(data);
    }
  } catch (error) {
    if (error.code === 'ENOENT') {
      console.log(`File '${fileName}' not found.`);
    } else {
      console.error(`An error occurred: ${err}`);
    }
  }

  try {
    const data = fs.readFileSync(errorLogFilePath, 'utf8');
    if (data.length) {
      console.error('There was an error creating the tunnel.');
      console.error(data);
      return false;
    }
  } catch (error) {
    if (error.code === 'ENOENT') {
      console.log(`File '${fileName}' not found.`);
    } else {
      console.error(`An error occurred: ${err}`);
    }
  }

  return true;
}

function main() {

  // Open a file to store the child process's output
  const outputLogFile = fs.openSync(outputLogFilePath, 'a');
  const errorLogFile = fs.openSync(errorLogFilePath, 'a');

  // Spawn a child process to run the shell script with stdin, stdout, and stderr redirected
  const childProcess = spawn('bash', [scriptPath], {
    detached: true,
    stdio: ['pipe', outputLogFile, errorLogFile], // Redirect output to log files
  });

  // Close the file descriptors to release them
  fs.closeSync(outputLogFile);
  fs.closeSync(errorLogFile);

  console.log('Starting tunnel...');
  // initial wait
  sleep(3000);
  const result = printOutput();

  if (result) {
    waitForDeviceVerification();
  }

  // Unref the child process to allow the Node.js process to exit
  childProcess.unref();
}

main();
