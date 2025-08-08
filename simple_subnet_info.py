#!/usr/bin/env python3
"""
Simple Bittensor subnet info getter
"""

import bittensor as bt
import sys

def get_simple_subnet_info(netuid):
    """Get basic information about a specific subnet"""
    
    print(f"🔗 Connecting to Bittensor network...")
    subtensor = bt.subtensor(network='finney')
    
    try:
        print(f"📋 Fetching subnet {netuid} information...")
        
        # Get subnet info
        subnet_info = subtensor.get_subnet_info(netuid)
        
        if subnet_info is None:
            print(f"❌ Subnet {netuid} does not exist")
            return
            
        print(f"\n📊 Subnet {netuid} Information:")
        print("=" * 50)
        print(f"Network UID: {subnet_info.netuid}")
        print(f"Owner: {subnet_info.owner_ss58}")
        print(f"Burn cost: {subnet_info.burn}")
        print(f"Difficulty: {subnet_info.difficulty}")
        print(f"Max neurons: {subnet_info.max_n}")
        print(f"Current neurons: {subnet_info.subnetwork_n}")
        print(f"Tempo: {subnet_info.tempo}")
        print(f"Immunity period: {subnet_info.immunity_period}")
        print(f"Min allowed weights: {subnet_info.min_allowed_weights}")
        print(f"Max weight limit: {subnet_info.max_weight_limit}")
        print(f"Emission value: {subnet_info.emission_value}")
        
    except Exception as e:
        print(f"❌ Error fetching subnet info: {e}")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python3 simple_subnet_info.py <netuid>")
        sys.exit(1)
    
    try:
        netuid = int(sys.argv[1])
        get_simple_subnet_info(netuid)
    except ValueError:
        print("❌ Invalid subnet ID. Please provide a number.")
        sys.exit(1)
