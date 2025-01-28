#!/bin/bash

set -e # Exit immediately if a command exits with a non-zero status
set -u # Treat unset vairables as an error and exit immediately

# Define paths and variables
APP_NAME="mail-forge"
REPO_DIR="/home/ubuntu/$APP_NAME"
PROJECT_DIR="$REPO_DIR/$APP_NAME"
TARGET_BIN="/usr/local/bin/$APP_NAME"
SYSTEMD_SERVICE_FILE="$REPO_DIR/$APPNAME.service"
SYSTEMD_DEST="/etc/systemd/system/$APP_NAME.service"
CONFIG_FILE_SOURCE="$REPO_DIR/config.toml"
CONFIG_FILE_DEST="$HOME/.config/mail-forge/config.toml"

confirm() {
	# Ask user for confirmation
	while true; do
		read -p "$1 [y/n]: " yn
		case $yn in
			[Yy]*) return 0 ;;
			[Nn]*) return 1 ;;
			*) echo "Please anser yes (y) or no (n)." ;;
		esac
	done
}

deploy_systemd() {
	sudo cp "$SYSTEMD_SERVICE_FILE" "$SYSTEMD_DEST"
	sudo systemctl daemon-reload
}

echo "Pulling the latest code..."
cd "$REPO_DIR"
git pull


echo "Building the application..."
cd "$PROJECT_DIR"
cargo build --release

# Check if the systemd service file exists
if [ -f "$SYSTEMD_DEST" ]; then
	if confirm "The systemd service file already exists. Do you want to overwrite it?"; then
		echo "Overwriting systemd service file..."
		deploy_systemd
	else
		echo "Skipping systemd service file deployment."
	fi
else
	echo "Deploying systemd service file..."
	deploy_systemd
fi

# Ensure the configuration directory exists
echo "Ensuring configuration directory exists..."
mkdir -p "$(dirname "$CONFIG_FILE_DEST")"

# Check if the configuration file exists
if [ -f "$CONFIG_FILE_DEST" ]; then
	if confirm "The configuration file already exists. Do you want to overwrite it?"; then
		echo "Overwriting configuration file..."
		cp "$CONFIG_FILE_SOURCE" "$CONFIG_FILE_DEST"
	else
		echo "Skipping configuration file deployment."
	fi
else
	echo "Deploying configuration file..."
	cp "$CONFIG_FILE_SOURCE" "$CONFIG_FILE_DEST"
fi


echo "Stopping the service..."
sudo systemctl stop "$APP_NAME.service" || echo "Service not running. Continuing..."

echo "Deploying the new binary..."
sudo cp "$PROJECT_DIR/target/release/$APP_NAME" "$TARGET_BIN"

echo "Setting capabilities on the binary..."
sudo setcap 'cap_net_bind_service=+ep' "$TARGET_BIN"

echo "Starting the service..."
sudo systemctl start "$APP_NAME.service"

echo "Deployment completed successfully!"
