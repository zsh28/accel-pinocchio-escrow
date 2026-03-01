#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use litesvm::LiteSVM;
    use litesvm_token::{
        spl_token::{self},
        CreateAssociatedTokenAccount, CreateMint, MintTo,
    };
    use solana_instruction::{AccountMeta, Instruction};
    use solana_keypair::Keypair;
    use solana_message::Message;
    use solana_native_token::LAMPORTS_PER_SOL;
    use solana_pubkey::Pubkey;
    use solana_signer::Signer;
    use solana_transaction::Transaction;
    use wincode::SchemaWrite;

    const PROGRAM_ID: &str = "4ibrEMW5F6hKnkW4jVedswYv6H6VtwPN6ar6dvXDN1nT";
    const TOKEN_PROGRAM_ID: Pubkey = spl_token::ID;
    const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

    #[derive(SchemaWrite)]
    struct MakeV2InstructionData {
        amount_to_receive: u64,
        amount_to_give: u64,
        bump: u8,
    }

    struct MakeSetup {
        svm: LiteSVM,
        maker: Keypair,
        mint_a: Pubkey,
        mint_b: Pubkey,
        escrow: (Pubkey, u8),
        maker_ata_a: Pubkey,
        vault: Pubkey,
    }

    fn program_id() -> Pubkey {
        Pubkey::from(crate::ID)
    }

    fn setup() -> (LiteSVM, Keypair) {
        let mut svm = LiteSVM::new();
        let payer = Keypair::new();

        svm.airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL)
            .expect("airdrop failed");

        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let candidates = [
            base.join("target/sbpf-solana-solana/release/escrow.so"),
            base.join("target/sbf-solana-solana/release/escrow.so"),
            base.join("target/deploy/escrow.so"),
        ];

        let so_path = candidates
            .iter()
            .find(|path| path.exists())
            .unwrap_or_else(|| panic!("program .so not found in expected target paths"));

        let program_data = std::fs::read(so_path).expect("failed to read program SO file");
        svm.add_program(program_id(), &program_data)
            .expect("failed to add program");

        (svm, payer)
    }

    fn setup_make(v2: bool, amount_to_receive: u64, amount_to_give: u64) -> MakeSetup {
        let (mut svm, maker) = setup();

        let mint_a = CreateMint::new(&mut svm, &maker)
            .decimals(6)
            .authority(&maker.pubkey())
            .send()
            .unwrap();

        let mint_b = CreateMint::new(&mut svm, &maker)
            .decimals(6)
            .authority(&maker.pubkey())
            .send()
            .unwrap();

        let maker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
            .owner(&maker.pubkey())
            .send()
            .unwrap();

        let escrow =
            Pubkey::find_program_address(&[b"escrow", maker.pubkey().as_ref()], &program_id());
        let vault = spl_associated_token_account::get_associated_token_address(&escrow.0, &mint_a);

        MintTo::new(&mut svm, &maker, &mint_a, &maker_ata_a, amount_to_give)
            .send()
            .unwrap();

        let data = if v2 {
            let mut out = vec![3u8];
            let serialized = wincode::serialize(&MakeV2InstructionData {
                amount_to_receive,
                amount_to_give,
                bump: escrow.1,
            })
            .unwrap();
            out.extend_from_slice(&serialized);
            out
        } else {
            [
                vec![0u8],
                vec![escrow.1],
                amount_to_receive.to_le_bytes().to_vec(),
                amount_to_give.to_le_bytes().to_vec(),
            ]
            .concat()
        };

        let make_ix = Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(maker.pubkey(), true),
                AccountMeta::new(mint_a, false),
                AccountMeta::new(mint_b, false),
                AccountMeta::new(escrow.0, false),
                AccountMeta::new(maker_ata_a, false),
                AccountMeta::new(vault, false),
                AccountMeta::new(solana_sdk_ids::system_program::ID, false),
                AccountMeta::new(TOKEN_PROGRAM_ID, false),
                AccountMeta::new(
                    ASSOCIATED_TOKEN_PROGRAM_ID.parse::<Pubkey>().unwrap(),
                    false,
                ),
            ],
            data,
        };

        let message = Message::new(&[make_ix], Some(&maker.pubkey()));
        let recent_blockhash = svm.latest_blockhash();
        let transaction = Transaction::new(&[&maker], message, recent_blockhash);
        svm.send_transaction(transaction).unwrap();

        MakeSetup {
            svm,
            maker,
            mint_a,
            mint_b,
            escrow,
            maker_ata_a,
            vault,
        }
    }

    fn run_take(v2: bool) {
        let amount_to_receive: u64 = 100_000_000;
        let amount_to_give: u64 = 500_000_000;
        let mut s = setup_make(v2, amount_to_receive, amount_to_give);

        let taker = Keypair::new();
        s.svm
            .airdrop(&taker.pubkey(), 10 * LAMPORTS_PER_SOL)
            .unwrap();

        let taker_ata_a = CreateAssociatedTokenAccount::new(&mut s.svm, &taker, &s.mint_a)
            .owner(&taker.pubkey())
            .send()
            .unwrap();

        let taker_ata_b = CreateAssociatedTokenAccount::new(&mut s.svm, &taker, &s.mint_b)
            .owner(&taker.pubkey())
            .send()
            .unwrap();

        let maker_ata_b = CreateAssociatedTokenAccount::new(&mut s.svm, &taker, &s.mint_b)
            .owner(&s.maker.pubkey())
            .send()
            .unwrap();

        MintTo::new(
            &mut s.svm,
            &s.maker,
            &s.mint_b,
            &taker_ata_b,
            amount_to_receive,
        )
        .send()
        .unwrap();

        let data = if v2 { vec![4u8] } else { vec![1u8] };

        let take_ix = Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(taker.pubkey(), true),
                AccountMeta::new(s.maker.pubkey(), false),
                AccountMeta::new(s.escrow.0, false),
                AccountMeta::new(taker_ata_a, false),
                AccountMeta::new(taker_ata_b, false),
                AccountMeta::new(maker_ata_b, false),
                AccountMeta::new(s.vault, false),
                AccountMeta::new(TOKEN_PROGRAM_ID, false),
            ],
            data,
        };

        let message = Message::new(&[take_ix], Some(&taker.pubkey()));
        let recent_blockhash = s.svm.latest_blockhash();
        let transaction = Transaction::new(&[&taker], message, recent_blockhash);
        s.svm.send_transaction(transaction).unwrap();
    }

    fn run_cancel(v2: bool) {
        let amount_to_receive: u64 = 100_000_000;
        let amount_to_give: u64 = 500_000_000;
        let mut s = setup_make(v2, amount_to_receive, amount_to_give);

        let data = if v2 { vec![5u8] } else { vec![2u8] };

        let cancel_ix = Instruction {
            program_id: program_id(),
            accounts: vec![
                AccountMeta::new(s.maker.pubkey(), true),
                AccountMeta::new(s.escrow.0, false),
                AccountMeta::new(s.maker_ata_a, false),
                AccountMeta::new(s.vault, false),
                AccountMeta::new(TOKEN_PROGRAM_ID, false),
            ],
            data,
        };

        let message = Message::new(&[cancel_ix], Some(&s.maker.pubkey()));
        let recent_blockhash = s.svm.latest_blockhash();
        let transaction = Transaction::new(&[&s.maker], message, recent_blockhash);
        s.svm.send_transaction(transaction).unwrap();
    }

    #[test]
    fn test_make_v1() {
        let pid = program_id();
        assert_eq!(pid.to_string(), PROGRAM_ID);
        let _ = setup_make(false, 100_000_000, 500_000_000);
    }

    #[test]
    fn test_take_v1() {
        run_take(false);
    }

    #[test]
    fn test_cancel_v1() {
        run_cancel(false);
    }

    #[test]
    fn test_make_v2() {
        let _ = setup_make(true, 100_000_000, 500_000_000);
    }

    #[test]
    fn test_take_v2() {
        run_take(true);
    }

    #[test]
    fn test_cancel_v2() {
        run_cancel(true);
    }
}
