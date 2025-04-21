use ldk_node::Builder;
use ldk_node::Event;
use ldk_node::bitcoin::Network;
use ldk_node::bitcoin::secp256k1::PublicKey;
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_node::lightning_invoice::{Bolt11Invoice, Bolt11InvoiceDescription, Description};
use lnurl::LnUrlResponse;
use lnurl::lightning_address::LightningAddress;
use std::str::FromStr;
use std::sync::Arc;
use std::thread::spawn;

fn main() -> anyhow::Result<()> {
    let mut rl = rustyline::DefaultEditor::new()?;

    let client = lnurl::Builder::default().build_blocking()?;

    let mut builder = Builder::new();

    builder.set_network(Network::Signet);
    builder.set_chain_source_esplora("https://mutinynet.com/api".to_string(), None);
    builder.set_gossip_source_rgs("https://rgs.mutinynet.com/snapshot".to_string());
    builder.set_liquidity_source_lsps2(
        PublicKey::from_str("0371d6fd7d75de2d0372d03ea00e8bacdacb50c27d0eaea0a76a0622eff1f5ef2b")?,
        SocketAddress::from_str("3.84.56.108:39735").unwrap(),
        Some("4GH1W3YW".to_string()),
    );

    let node = builder.build().unwrap();
    let node = Arc::new(node);

    node.start().unwrap();

    let event_node = node.clone();
    spawn(move || {
        loop {
            match event_node.next_event() {
                None => continue,
                Some(Event::PaymentReceived {
                    payment_id,
                    payment_hash,
                    amount_msat,
                    custom_records,
                }) => {
                    println!("Received {amount_msat} msats!");
                    event_node.event_handled().unwrap();
                }
                Some(Event::PaymentClaimable {
                    payment_id,
                    payment_hash,
                    claimable_amount_msat,
                    claim_deadline,
                    custom_records,
                }) => {
                    println!("Claimable payment: {}", payment_hash);

                    event_node.event_handled().unwrap();
                }
                Some(Event::PaymentSuccessful {
                    payment_id,
                    payment_hash,
                    payment_preimage,
                    fee_paid_msat,
                }) => {
                    println!("Payment success! {payment_id:?}");
                    event_node.event_handled().unwrap();
                }
                Some(Event::PaymentFailed {
                    payment_id,
                    payment_hash,
                    reason,
                }) => {
                    println!("Payment failed :( {payment_id:?}");
                    event_node.event_handled().unwrap();
                }
                Some(Event::ChannelPending {
                    channel_id,
                    user_channel_id,
                    former_temporary_channel_id,
                    counterparty_node_id,
                    funding_txo,
                }) => {
                    println!("Channel Pending: {}", funding_txo.txid);
                    event_node.event_handled().unwrap();
                }
                Some(Event::ChannelReady {
                    channel_id,
                    user_channel_id,
                    counterparty_node_id,
                }) => {
                    println!("Channel Ready: {channel_id:?}");
                    event_node.event_handled().unwrap();
                }
                Some(_) => continue,
            }
        }
    });

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let words = line.trim().split_whitespace().collect::<Vec<_>>();
                if words.is_empty() {
                    continue;
                }

                match words[0] {
                    "exit" => {
                        break;
                    }
                    "address" => {
                        let addr = node.onchain_payment().new_address().unwrap();
                        println!("{}", addr);
                    }
                    "balance" => {
                        let balance = node.list_balances();
                        println!("{:?}", balance);
                    }
                    "sync" => {
                        node.sync_wallets().unwrap();
                        println!("SYNCED!");
                    }
                    "open" => {
                        if words.len() < 4 {
                            println!("Invalid syntax");
                            continue;
                        }
                        let node_id = PublicKey::from_str(words[1]).unwrap();
                        let address = SocketAddress::from_str(words[2]).unwrap();
                        let amount = u64::from_str(words[3]).unwrap();
                        let chan_id = node
                            .open_channel(node_id, address, amount, None, None)
                            .unwrap();
                        println!("Channel opening! {chan_id:?}");
                    }
                    "send" => {
                        if words.len() < 3 {
                            println!("Invalid syntax");
                            continue;
                        }

                        let ln_addr = LightningAddress::from_str(words[1]).unwrap();
                        let amount = u64::from_str(words[2]).unwrap();

                        let lnurl = ln_addr.lnurl();

                        let response = client.make_request(&lnurl.url).unwrap();

                        match response {
                            LnUrlResponse::LnUrlPayResponse(pay) => {
                                if pay.max_sendable < amount * 1000
                                    || pay.min_sendable > amount * 1000
                                {
                                    println!("Invalid amount");
                                    continue;
                                }

                                let invoice =
                                    client.get_invoice(&pay, amount * 1000, None, None).unwrap();

                                let bolt11 = Bolt11Invoice::from_str(invoice.invoice()).unwrap();

                                let payment = node.bolt11_payment().send(&bolt11, None).unwrap();

                                println!("Payment started! {payment}");
                            }
                            LnUrlResponse::LnUrlWithdrawResponse(_) => {
                                println!("Error wrong response!");
                                continue;
                            }
                            LnUrlResponse::LnUrlChannelResponse(_) => {
                                println!("Error wrong response!");
                                continue;
                            }
                        }
                    }
                    "channels" => {
                        let channels = node.list_channels();
                        println!("{}", channels.len());
                    }
                    "receive" => {
                        if words.len() < 2 {
                            println!("Invalid syntax");
                            continue;
                        }
                        let amount = u64::from_str(words[1]).unwrap() * 1000;
                        let desc = Bolt11InvoiceDescription::Direct(Description::default());
                        let invoice = node.bolt11_payment().receive(amount, &desc, 3600).unwrap();

                        println!("{invoice}");
                    }
                    "lsprecv" => {
                        if words.len() < 2 {
                            println!("Invalid syntax");
                            continue;
                        }
                        let amount = u64::from_str(words[1]).unwrap() * 1000;
                        let desc = Bolt11InvoiceDescription::Direct(Description::default());
                        let invoice = node
                            .bolt11_payment()
                            .receive_via_jit_channel(amount, &desc, 3600, None)
                            .unwrap();

                        println!("{invoice}");
                    }
                    &_ => {
                        println!("Unknown command: {}", words[0]);
                    }
                }
            }
            Err(_) => break,
        }
    }

    node.stop().unwrap();

    Ok(())
}
