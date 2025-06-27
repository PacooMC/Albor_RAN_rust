// Add subscriber for IMSI 999700000000001
db = db.getSiblingDB('open5gs');

// Remove any existing subscriber
db.subscribers.deleteMany({imsi: "999700000000001"});

// Add new subscriber
db.subscribers.insertOne({
  imsi: "999700000000001",
  subscribed_rau_tau_timer: 12,
  network_access_mode: 0,
  subscriber_status: 0,
  access_restriction_data: 32,
  slice: [{
    sst: 1,
    default_indicator: true,
    session: [{
      name: "internet",
      type: 3,
      pcc_rule: [],
      ambr: {
        uplink: { value: 1, unit: 3 },
        downlink: { value: 1, unit: 3 }
      },
      qos: {
        index: 9,
        arp: {
          priority_level: 8,
          pre_emption_capability: 1,
          pre_emption_vulnerability: 1
        }
      }
    }]
  }],
  ambr: {
    uplink: { value: 1, unit: 3 },
    downlink: { value: 1, unit: 3 }
  },
  security: {
    k: "465B5CE8B199B49FAA5F0A2EE238A6BC",
    amf: "8000",
    op: null,
    opc: "E8ED289DEBA952E4283B54E88E6183CA",
    sqn: NumberLong("0")
  },
  schema_version: 1,
  __v: 0
});

print("Subscriber added. Count: " + db.subscribers.count());