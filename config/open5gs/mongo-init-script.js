// MongoDB initialization script for Open5GS
// This script creates the initial database structure and adds a test subscriber

// Create open5gs database and user
db = db.getSiblingDB('open5gs');

// Create the subscribers collection
db.createCollection('subscribers');

// Add a test subscriber matching our UE configuration
db.subscribers.insertOne({
  imsi: '001010000000001',
  msisdn: [],
  imeisv: '3534900698733190',
  mme_host: [],
  mme_realm: [],
  purge_flag: [],
  security: {
    k: '00112233445566778899AABBCCDDEEFF',
    op: null,
    opc: '63BFA50EE6523365FF14C1F45F88737D',
    amf: '8000',
    sqn: NumberLong("0")
  },
  ambr: {
    downlink: { value: 1, unit: 3 },
    uplink: { value: 1, unit: 3 }
  },
  slice: [{
    sst: 1,
    sd: '000001',
    default_indicator: true,
    session: [{
      name: 'internet',
      type: 3,  // IPv4v6
      qos: {
        index: 9,
        arp: {
          priority_level: 8,
          pre_emption_capability: 1,
          pre_emption_vulnerability: 1
        }
      },
      ambr: {
        downlink: { value: 1, unit: 3 },
        uplink: { value: 1, unit: 3 }
      },
      pcc_rule: []
    }]
  }],
  access_restriction_data: 32,
  network_access_mode: 0,
  subscriber_status: 0,
  operator_determined_barring: 0,
  schema_version: 1
});

// Create indexes for performance
db.subscribers.createIndex({ imsi: 1 }, { unique: true });

print('Open5GS database initialized with test subscriber IMSI: 001010000000001');