#!/usr/bin/env python3
"""
Get Bittensor subnet information using the official Bittensor Python API
"""

import bittensor as bt
import sys

def get_subnet_info(netuid):
    """Get information about a specific subnet"""
    
    # Create a subtensor connection
    print(f"🔗 Connecting to Bittensor network...")
    subtensor = bt.subtensor(network='finney')  # or 'test' for testnet
    
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
        print(f"Burn cost: {subnet_info.burn}")
        print(f"Difficulty: {subnet_info.difficulty}")
        print(f"Immunity period: {subnet_info.immunity_period}")
        print(f"Min allowed weights: {subnet_info.min_allowed_weights}")
        print(f"Max weight limit: {subnet_info.max_weight_limit}")
        print(f"Max allowed validators: {subnet_info.max_allowed_validators}")
        print(f"Max neurons: {subnet_info.max_n}")
        print(f"Tempo: {subnet_info.tempo}")
        print(f"Modality: {subnet_info.modality}")
        print(f"Owner hotkey: {subnet_info.owner_ss58}")
        print(f"Emission value: {subnet_info.emission_value}")
        print(f"Rho: {subnet_info.rho}")
        print(f"Kappa: {subnet_info.kappa}")
        print(f"Scaling law power: {subnet_info.scaling_law_power}")
        print(f"Subnetwork N: {subnet_info.subnetwork_n}")
        print(f"Blocks since epoch: {subnet_info.blocks_since_epoch}")
        
        # Get registration cost in RAO and estimate in USD
        burn_cost = float(subnet_info.burn)
        print(f"\n💰 Registration Details:")
        print(f"Burn cost: {burn_cost:,.0f} RAO")
        
        # Get current TAO price (you might need to fetch this from an API)
        # For now, let's use a placeholder
        tao_price_usd = 200  # You can update this with real-time price
        rao_to_tao = 1e9
        burn_cost_tao = burn_cost / rao_to_tao
        burn_cost_usd = burn_cost_tao * tao_price_usd
        print(f"Burn cost: {burn_cost_tao:.6f} TAO (~${burn_cost_usd:.2f} USD)")
        
        # Get subnet participants
        try:
            metagraph = subtensor.metagraph(netuid)
            print(f"\n👥 Network Participants:")
            print(f"Total neurons: {len(metagraph.hotkeys)}")
            print(f"Total stake: {sum(metagraph.S).item():.2f} TAO")
        except Exception as e:
            print(f"⚠️  Could not fetch metagraph: {e}")
            
    except Exception as e:
        print(f"❌ Error fetching subnet info: {e}")

def list_all_subnets():
    """List all available subnets"""
    
    print(f"🔗 Connecting to Bittensor network...")
    subtensor = bt.subtensor(network='finney')
    
    try:
        # Get all subnets info
        all_subnets_info = subtensor.get_all_subnets_info()
        
        print(f"\n📊 All Active Subnets:")
        print("=" * 80)
        print(f"{'NetUID':<8} {'Owner':<20} {'Neurons':<10} {'Burn Cost':<15} {'Difficulty':<12}")
        print("-" * 80)
        
        for subnet_info in all_subnets_info:
            try:
                netuid = subnet_info.netuid
                metagraph = subtensor.metagraph(netuid)
                
                owner_short = subnet_info.owner_ss58[:8] + "..." + subnet_info.owner_ss58[-8:]
                neuron_count = len(metagraph.hotkeys)
                burn_cost_str = str(subnet_info.burn)
                difficulty = subnet_info.difficulty
                
                print(f"{netuid:<8} {owner_short:<20} {neuron_count:<10} {burn_cost_str:<15} {difficulty:<12}")
                
            except Exception as e:
                print(f"{netuid:<8} {'Error':<20} {'N/A':<10} {'N/A':<15} {'N/A':<12}")
                
    except Exception as e:
        print(f"❌ Error listing subnets: {e}")

if __name__ == "__main__":
    if len(sys.argv) == 1:
        list_all_subnets()
    elif len(sys.argv) == 2:
        try:
            netuid = int(sys.argv[1])
            get_subnet_info(netuid)
        except ValueError:
            print("❌ Invalid subnet ID. Please provide a number.")
            sys.exit(1)
    else:
        print("Usage:")
        print("  python3 get_subnet_info.py          # List all subnets")
        print("  python3 get_subnet_info.py <netuid> # Get specific subnet info")
