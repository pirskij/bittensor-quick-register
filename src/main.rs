use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::time::Duration;
use tokio::time::sleep;

pub mod utils;
pub mod key_utils;
pub mod client;
pub mod register;
pub mod constants;

use crate::register::*;

#[derive(Parser)]
#[command(name = "bittensor-quick-register")]
#[command(about = "Quick registration tool for Bittensor network")]
struct Cli {
    /// RPC endpoint URL
    #[arg(short = 'r', long, default_value = "wss://entrypoint-finney.opentensor.ai:443")]
    rpc_url: String,
 
    #[command(subcommand)]
    command: Commands,
}

// Дополним CLI с правильными commands
impl Cli {
    fn parse() -> Self {
        use clap::Parser;
        let mut cli = <Cli as Parser>::parse();
        
        // Parse subcommands
        let matches = clap::Command::new("bittensor-quick-register")
            .subcommand_required(true)
            .subcommand(
                clap::Command::new("register")
                    .about("Register to a subnet")
                    .arg(clap::Arg::new("subnet").short('s').long("subnet").required(true))
                    .arg(clap::Arg::new("wallet").short('w').long("wallet").required(true))
                    .arg(clap::Arg::new("hotkey").short('H').long("hotkey").required(true))
                    .arg(clap::Arg::new("burn_amount").long("burn-amount"))
            )
            .subcommand(
                clap::Command::new("status")
                    .about("Check registration status")
                    .arg(clap::Arg::new("subnet").short('s').long("subnet").required(true))
                    .arg(clap::Arg::new("hotkey").short('H').long("hotkey").required(true))
            )
            .subcommand(
                clap::Command::new("info")
                    .about("Show subnet information")
                    .arg(clap::Arg::new("subnet").short('s').long("subnet").required(true))
            )
            .subcommand(
                clap::Command::new("estimate")
                    .about("Estimate registration costs")
                    .arg(clap::Arg::new("subnet").short('s').long("subnet").required(true))
            )
            .get_matches();
            
        cli.command = match matches.subcommand() {
            Some(("register", sub_m)) => Commands::Register {
                subnet: sub_m.get_one::<String>("subnet").unwrap().parse().unwrap(),
                wallet: sub_m.get_one::<String>("wallet").unwrap().clone(),
                hotkey: sub_m.get_one::<String>("hotkey").unwrap().clone(),
                burn_amount: sub_m.get_one::<String>("burn_amount").map(|s| s.parse().unwrap()),
            },
            Some(("status", sub_m)) => Commands::Status {
                subnet: sub_m.get_one::<String>("subnet").unwrap().parse().unwrap(),  
                hotkey: sub_m.get_one::<String>("hotkey").unwrap().clone(),
            },
            Some(("info", sub_m)) => Commands::SubnetInfo {
                subnet: sub_m.get_one::<String>("subnet").unwrap().parse().unwrap(),
            },
            Some(("estimate", sub_m)) => Commands::EstimateCost {
                subnet: sub_m.get_one::<String>("subnet").unwrap().parse().unwrap(),
            },
            _ => unreachable!(),
        };
        
        cli
    }
}
 
#[derive(Subcommand)]
enum Commands {
    /// Register to a subnet using PoW or burn registration
    Register {
        #[arg(short, long)]
        subnet: u16,
        #[arg(short, long)]
        wallet: String,
        #[arg(short = 'H', long)]
        hotkey: String,
        #[arg(long)]
        burn_amount: Option<u64>,
    },
    
    /// Check registration status of a hotkey
    Status {
        #[arg(short, long)]
        subnet: u16,
        #[arg(short = 'H', long)]
        hotkey: String,
    },
    
    /// Show detailed subnet information
    SubnetInfo {
        #[arg(short, long)]
        subnet: u16,
    },
    
    /// Estimate registration costs and time
    EstimateCost {
        #[arg(short, long)]
        subnet: u16,
    },
    
    /// Monitor multiple neurons across subnets
    Monitor {
        #[arg(short, long, help = "Format: subnet1:hotkey1,subnet2:hotkey2")]
        neurons: Vec<String>,
        #[arg(long, default_value = "60")]
        interval: u64,
    },
    
    /// Auto-register with retry logic
    AutoRegister {
        #[arg(short, long)]
        subnet: u16,
        #[arg(short, long)]
        wallet: String,
        #[arg(short = 'H', long)]
        hotkey: String,
        #[arg(long, default_value = "3")]
        max_retries: usize,
    },
    
    /// Show network statistics
    NetworkStats,
    
    /// Export subnet configuration
    ExportConfig {
        #[arg(short, long)]
        subnet: u16,
        #[arg(short, long, default_value = "subnet_config.json")]
        output: String,
    },
    
    /// Batch operations from config file
    Batch {
        #[arg(short, long)]
        config: String,
    },
}
 
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    )
    .format_timestamp(None)
    .format_module_path(false)
    .init();
    
    print_banner();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Register { 
            subnet, wallet, hotkey, burn_amount 
        } => {
            let register_client = QuickRegister::new(cli.rpc_url).await?;
            register_client.register_to_subnet(
                subnet, &wallet, &hotkey, burn_amount
            ).await?;
        }
        
        Commands::Status { subnet, hotkey } => {
            let register_client = QuickRegister::new(cli.rpc_url).await?;
            register_client.check_status(subnet, &hotkey).await?;
        }
        
        Commands::SubnetInfo { subnet } => {
            let register_client = QuickRegister::new(cli.rpc_url).await?;
            register_client.show_subnet_info(subnet).await?;
        }
        
        Commands::EstimateCost { subnet } => {
            let register_client = QuickRegister::new(cli.rpc_url).await?;
            register_client.estimate_registration_cost(subnet).await?;
        }
        
        Commands::Monitor { neurons, interval } => {
            let register_client = QuickRegister::new(cli.rpc_url).await?;
            let parsed_neurons: Result<Vec<(u16, String)>> = neurons
                .iter()
                .map(|s| {
                    let parts: Vec<&str> = s.split(':').collect();
                    if parts.len() == 2 {
                        Ok((parts[0].parse::<u16>()?, parts[1].to_string()))
                    } else {
                        Err(anyhow!("Invalid format: {}. Use subnet:hotkey", s))
                    }
                })
                .collect();
            
            let parsed_neurons = parsed_neurons?;
            
            loop {
                register_client.monitor_multiple_neurons(parsed_neurons.clone()).await?;
                println!("\n⏳ Waiting {}s before next check...", interval);
                sleep(Duration::from_secs(interval)).await;
            }
        }
        
        Commands::AutoRegister { subnet, wallet, hotkey, max_retries } => {
            let register_client = QuickRegister::new(cli.rpc_url).await?;
            register_client.auto_register_with_retry(subnet, &wallet, &hotkey, max_retries).await?;
        }
        
        Commands::NetworkStats => {
            let register_client = QuickRegister::new(cli.rpc_url).await?;
            register_client.show_network_statistics().await?;
        }
        
        Commands::ExportConfig { subnet, output } => {
            let register_client = QuickRegister::new(cli.rpc_url).await?;
            register_client.export_config(subnet, &output).await?;
        }
        
        Commands::Batch { config } => {
            let register_client = QuickRegister::new(cli.rpc_url).await?;
            register_client.execute_batch_operations(&config).await?;
        }
    }
    
    Ok(())
}

fn print_banner() {
    println!("{}", r#"
 ╔═══════════════════════════════════════════════════════════╗
 ║                                                           ║
 ║    🚀 Bittensor Quick Register v0.1.0                     ║
 ║    ⚡ Fast • Reliable • Burn Registration                 ║
 ║                                                           ║
 ╚═══════════════════════════════════════════════════════════╝
    "#.bright_cyan());
}
