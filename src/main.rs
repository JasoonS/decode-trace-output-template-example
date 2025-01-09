use alloy_dyn_abi::DynSolValue;
use alloy_json_abi::Function;
use anyhow::{Context, Result};
use hypersync_client::{
    net_types::Query, simple_types::Trace, ArrowResponseData, CallDecoder, Client, ClientConfig,
    FromArrow, StreamConfig,
};
use std::sync::Arc;

const GET_USER_ACCOUNT_DATA_SIGNATURE: &str = "function getUserAccountData(address user) external view returns (uint256 totalCollateralBase, uint256 totalDebtBase, uint256 availableBorrowsBase, uint256 currentLiquidationThreshold, uint256 ltv, uint256 healthFactor)";

const AAVE_V3_POOL_ADDRESS: &str = "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2";

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let client = Arc::new(
        Client::new(ClientConfig {
            max_num_retries: Some(10),
            ..ClientConfig::default()
        })
            .unwrap(),
    );

    let signature = Function::parse(GET_USER_ACCOUNT_DATA_SIGNATURE.as_ref())
        .context("parse function signature")?
        .selector();

    let query: Query = serde_json::from_value(serde_json::json!({
        "from_block": 16291127, // Aave V3 deployment block
        "traces": [{
            "sighash": [format!("{:}", signature)],
            "to": [AAVE_V3_POOL_ADDRESS],
        }],
        "field_selection": {
            "trace": ["input", "output", "block_number"],
            // "block": ["number", "timestamp"],
        }
    }))
        .unwrap();
    let decoder = CallDecoder::from_signatures(&[GET_USER_ACCOUNT_DATA_SIGNATURE]).unwrap();

    let config = StreamConfig {
        ..Default::default()
    };

    let mut rx = client.clone().stream_arrow(query, config).await?;

    fn convert_traces(arrow_response_data: ArrowResponseData) -> Vec<Trace> {
        arrow_response_data
            .traces
            .iter()
            .flat_map(Trace::from_arrow)
            .collect()
    }

    while let Some(result) = rx.recv().await {
        match result {
            Ok(response) => {
                println!("Received response");
                let traces = convert_traces(response.data);
                for trace in traces {
                    if let (Some(input), Some(output), Some(block_number)) =
                        (trace.input, trace.output, trace.block_number)
                    {
                        if let Some(args) = decoder
                            .decode_input(&input)
                            .context("Failed to decode input")?
                        {
                            if let Some(&DynSolValue::Address(user_address)) = args.get(0) {
                                println!("Block: {}", block_number);
                                println!("User: {:#x}", user_address);
                                println!("Raw output: 0x{}", hex::encode(&output));
                                // TODO: Decode the output
                                println!("----------------------------------------");
                                if let Some(results) = decoder.decode_output(&output, GET_USER_ACCOUNT_DATA_SIGNATURE).context("Failed to decode output")? {
                                    let (total_collateral_base,_) = results[0].as_uint().unwrap();
                                    let (total_debt_base,_) = results[1].as_uint().unwrap();
                                    let (available_borrows_base,_) = results[2].as_uint().unwrap();
                                    let (current_liquidation_threshold,_) = results[3].as_uint().unwrap();
                                    let (ltv,_) = results[4].as_uint().unwrap();
                                    let (health_factor,_) = results[5].as_uint().unwrap();

                                    println!("total_collateral_base     {}", total_collateral_base);
                                    println!("total_debt_base       {}", total_debt_base);
                                    println!("available_borrows_base     {}", available_borrows_base);
                                    println!("current_liquidation_threshold     {}", current_liquidation_threshold);
                                    println!("ltv     {}", ltv );
                                    println!("health_factor     {}", health_factor);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {:?}", e);
            }
        }
    }

    Ok(())
}
