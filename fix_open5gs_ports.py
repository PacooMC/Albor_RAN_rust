#!/usr/bin/env python3
import yaml
import sys
import os

# Port assignments
port_mapping = {
    'amf': 7777,
    'smf': 7778,
    'pcf': 7779,
    'udr': 7780,
    'udm': 7781,
    'ausf': 7782,
    'nrf': 7783,
    'bsf': 7784,
    'nssf': 7785,
    'scp': 7786
}

config_dir = '/opt/open5gs/etc/open5gs'

def fix_config_file(service_name):
    """Fix configuration for a specific service"""
    file_path = os.path.join(config_dir, f'{service_name}.yaml')
    
    if not os.path.exists(file_path):
        print(f"Skipping {file_path} - does not exist")
        return
        
    try:
        # Read the YAML file
        with open(file_path, 'r') as f:
            config = yaml.safe_load(f)
        
        # Update SBI server port if it exists
        if service_name in config and 'sbi' in config[service_name]:
            if 'server' in config[service_name]['sbi']:
                for server in config[service_name]['sbi']['server']:
                    if 'port' in server:
                        server['port'] = port_mapping.get(service_name, 7777)
                        print(f"Updated {service_name} SBI server port to {server['port']}")
            
            # Update client references
            if 'client' in config[service_name]['sbi']:
                # Update NRF references
                if 'nrf' in config[service_name]['sbi']['client']:
                    for nrf in config[service_name]['sbi']['client']['nrf']:
                        if 'uri' in nrf:
                            nrf['uri'] = nrf['uri'].replace(':7777', f':{port_mapping["nrf"]}')
                            print(f"Updated {service_name} NRF client URI to {nrf['uri']}")
                
                # Update SCP references
                if 'scp' in config[service_name]['sbi']['client']:
                    for scp in config[service_name]['sbi']['client']['scp']:
                        if 'uri' in scp:
                            scp['uri'] = scp['uri'].replace(':7777', f':{port_mapping["scp"]}')
                            scp['uri'] = scp['uri'].replace(':7783', f':{port_mapping["scp"]}')  # Fix wrong port
                            print(f"Updated {service_name} SCP client URI to {scp['uri']}")
        
        # Write the updated YAML file
        with open(file_path, 'w') as f:
            yaml.dump(config, f, default_flow_style=False, sort_keys=False)
            
    except Exception as e:
        print(f"Error processing {file_path}: {str(e)}")

def main():
    """Main function to fix all configuration files"""
    print("Fixing Open5GS port configurations...")
    
    # Fix all service configurations
    for service in port_mapping.keys():
        fix_config_file(service)
    
    # Also check for additional services that might exist
    additional_services = ['sepp1', 'sepp2', 'mme', 'sgwc', 'sgwu', 'hss', 'pcrf', 'upf']
    for service in additional_services:
        file_path = os.path.join(config_dir, f'{service}.yaml')
        if os.path.exists(file_path):
            print(f"Processing additional service: {service}")
            try:
                with open(file_path, 'r') as f:
                    config = yaml.safe_load(f)
                
                # Update any NRF/SCP references in the file
                config_str = yaml.dump(config)
                config_str = config_str.replace('http://127.0.0.1:7777', f'http://127.0.0.1:{port_mapping["nrf"]}')
                config_str = config_str.replace('http://127.0.0.10:7777', f'http://127.0.0.1:{port_mapping["nrf"]}')
                config_str = config_str.replace('http://127.0.0.22:7777', f'http://127.0.0.1:{port_mapping["scp"]}')
                
                config = yaml.safe_load(config_str)
                
                with open(file_path, 'w') as f:
                    yaml.dump(config, f, default_flow_style=False, sort_keys=False)
                    
            except Exception as e:
                print(f"Error processing {file_path}: {str(e)}")
    
    print("\nPort configuration update completed!")
    
    # Verify the changes
    print("\nVerifying port assignments:")
    for service, port in port_mapping.items():
        file_path = os.path.join(config_dir, f'{service}.yaml')
        if os.path.exists(file_path):
            try:
                with open(file_path, 'r') as f:
                    config = yaml.safe_load(f)
                if service in config and 'sbi' in config[service] and 'server' in config[service]['sbi']:
                    actual_port = config[service]['sbi']['server'][0].get('port', 'not found')
                    print(f"{service}: {actual_port} (expected: {port})")
            except:
                pass

if __name__ == '__main__':
    main()