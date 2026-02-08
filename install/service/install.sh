#!/usr/bin/env bash

result=${PWD##*/}  
if [ "$result" != "raspberry-guardian" ]; then
    echo "Please run this script from the root directory."
    exit 1
fi

echo
sudo mkdir -p /var/lib/raspberry
sudo chown marc:battery-guardian /var/lib/raspberry

echo "1 - copying battery-guardian to /usr/local/bin/ and setting permissions..."
sudo cp target/release/battery-guardian /usr/local/bin/
sudo chmod +x /usr/local/bin/battery-guardian
echo "" 

echo "2 - check if battery-guardian is in /usr/local/bin/ and has execute permissions..."
ls -l /usr/local/bin/battery-guardian
echo "" 

echo "3 - copy config file to /etc/battery-guardian/ and setting permissions..."
sudo mkdir -p /etc/battery-guardian
sudo cp config.toml /etc/battery-guardian/
sudo chmod 644 /etc/battery-guardian/config.toml
echo ""

echo "4 - copying battery-guardian.service to /etc/systemd/system/..."
sudo cp instal/service/battery-guardian.service /etc/systemd/system/
echo "" 

echo "5 - reloading systemd daemon and enabling battery-guardian service..."
sudo systemctl daemon-reload
sudo systemctl enable battery-guardian.service
sudo systemctl start battery-guardian.service
echo "" 

echo "6 - status of battery-guardian service..."
sudo systemctl status battery-guardian.service

