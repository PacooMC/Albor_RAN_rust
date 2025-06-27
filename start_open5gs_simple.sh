#!/bin/bash
# Simple Open5GS startup script

echo "Starting Open5GS components..."

# Start each component
cd /open5gs/install/bin

# Start NRF first
./open5gs-nrfd -D &
sleep 2

# Start other components
./open5gs-scpd -D &
./open5gs-amfd -D &
./open5gs-smfd -D &
./open5gs-upfd -D &
./open5gs-bsfd -D &
./open5gs-udmd -D &
./open5gs-udrd -D &
./open5gs-ausfd -D &
./open5gs-nssfd -D &
./open5gs-pcfd -D &

sleep 3

# Add test subscriber
mongosh 127.0.0.1:27017/open5gs --quiet --eval '
db.subscribers.deleteOne({ "imsi": "001010000000001" });
db.subscribers.insertOne({
    "imsi": "001010000000001",
    "msisdn": ["0000000001"],
    "imeisv": "353490069873319",
    "security": {
        "k": "465B5CE8B199B49FAA5F0A2EE238A6BC",
        "opc": "E8ED289DEBA952E4283B54E88E6183CA",
        "amf": "8000"
    },
    "ambr": {
        "downlink": { "value": 1, "unit": 3 },
        "uplink": { "value": 1, "unit": 3 }
    },
    "slice": [{
        "sst": 1,
        "default_indicator": true,
        "session": [{
            "name": "internet",
            "type": 3,
            "qos": { "index": 9 },
            "ambr": {
                "downlink": { "value": 1, "unit": 3 },
                "uplink": { "value": 1, "unit": 3 }
            }
        }]
    }],
    "access_restriction_data": 32,
    "subscribed_rau_tau_timer": 12,
    "network_access_mode": 0
});'

echo "Open5GS started. Checking status..."
ps aux | grep open5gs | grep -v grep