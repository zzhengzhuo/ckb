use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};
use ckb_hash::blake2b_256;
use ckb_types::{
    bytes::Bytes,
    core::{
        cell::CellMeta, Capacity, EpochNumberWithFraction, HeaderBuilder, ScriptHashType,
        TransactionBuilder,
    },
    packed::{CellOutput, OutPoint, Script, ScriptBuilder},
    prelude::*,
    H256,
};
use ckb_vm::{
    memory::{FLAG_DIRTY, FLAG_EXECUTABLE, FLAG_FREEZED, FLAG_WRITABLE},
    registers::{A0, A1, A2, A3, A4, A5, A7},
    CoreMachine, Error as VMError, Memory, Syscalls, RISCV_PAGESIZE,
};
use proptest::{collection::size_range, prelude::*};
use std::collections::HashMap;

use super::SCRIPT_VERSION;
use crate::syscalls::{tests::utils::*, *};

fn _test_load_cell_not_exist(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A3, 1); //index
    machine.set_register(A4, u64::from(Source::Transaction(SourceEntry::Input))); //source: 1 input
    machine.set_register(A7, LOAD_CELL_SYSCALL_NUMBER); // syscall number

    prop_assert!(machine
        .memory_mut()
        .store64(&size_addr, &(data.len() as u64))
        .is_ok());

    let output_cell_data = Bytes::from(data.to_owned());
    let output = build_cell_meta(100, output_cell_data);
    let input_cell_data: Bytes = data.iter().rev().cloned().collect();
    let input_cell = build_cell_meta(100, input_cell_data);
    let outputs = vec![output];
    let resolved_inputs = vec![input_cell];
    let resolved_cell_deps = vec![];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let data_loader = new_mock_data_loader();
    let mut load_cell = LoadCell::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
    );

    prop_assert!(load_cell.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(INDEX_OUT_OF_BOUND));
    Ok(())
}

proptest! {
    #[test]
    fn test_load_cell_not_exist(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_cell_not_exist(data)?;
    }
}

fn _test_load_cell_all(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A3, 0); //index
    machine.set_register(A4, u64::from(Source::Transaction(SourceEntry::Input))); //source: 1 input
    machine.set_register(A7, LOAD_CELL_SYSCALL_NUMBER); // syscall number

    let output_cell_data = Bytes::from(data.to_owned());
    let output = build_cell_meta(100, output_cell_data);
    let input_cell_data: Bytes = data.iter().rev().cloned().collect();
    let input_cell = build_cell_meta(100, input_cell_data);
    let outputs = vec![output.clone()];
    let resolved_inputs = vec![input_cell.clone()];
    let resolved_cell_deps = vec![];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let data_loader = new_mock_data_loader();
    let mut load_cell = LoadCell::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
    );

    let input_correct_data = input_cell.cell_output.as_slice();
    let output_correct_data = output.cell_output.as_slice();

    // test input
    prop_assert!(machine
        .memory_mut()
        .store64(&size_addr, &(input_correct_data.len() as u64))
        .is_ok());

    prop_assert!(load_cell.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(
        machine.memory_mut().load64(&size_addr),
        Ok(input_correct_data.len() as u64)
    );

    for (i, addr) in (addr..addr + input_correct_data.len() as u64).enumerate() {
        prop_assert_eq!(
            machine.memory_mut().load8(&addr),
            Ok(u64::from(input_correct_data[i]))
        );
    }

    // clean memory
    prop_assert!(machine.memory_mut().store_byte(0, 1100, 0).is_ok());

    // test output
    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A4, u64::from(Source::Transaction(SourceEntry::Output))); //source: 2 output
    prop_assert!(machine
        .memory_mut()
        .store64(&size_addr, &(output_correct_data.len() as u64 + 10))
        .is_ok());

    prop_assert!(load_cell.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(
        machine.memory_mut().load64(&size_addr),
        Ok(output_correct_data.len() as u64)
    );

    for (i, addr) in (addr..addr + output_correct_data.len() as u64).enumerate() {
        prop_assert_eq!(
            machine.memory_mut().load8(&addr),
            Ok(u64::from(output_correct_data[i]))
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn test_load_cell_all(ref tx in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_cell_all(tx)?;
    }
}

fn _test_load_cell_length(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A3, 0); //index
    machine.set_register(A4, u64::from(Source::Transaction(SourceEntry::Input))); //source: 1 input
    machine.set_register(A7, LOAD_CELL_SYSCALL_NUMBER); // syscall number

    let output_cell_data = Bytes::from(data.to_owned());
    let output = build_cell_meta(100, output_cell_data);
    let input_cell_data: Bytes = data.iter().rev().cloned().collect();
    let input_cell = build_cell_meta(100, input_cell_data);
    let outputs = vec![output];
    let resolved_inputs = vec![input_cell.clone()];
    let resolved_cell_deps = vec![];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let data_loader = new_mock_data_loader();
    let mut load_cell = LoadCell::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
    );

    let input_correct_data = input_cell.cell_output.as_slice();

    prop_assert!(machine.memory_mut().store64(&size_addr, &0).is_ok());

    prop_assert!(load_cell.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(
        machine.memory_mut().load64(&size_addr),
        Ok(input_correct_data.len() as u64)
    );
    Ok(())
}

proptest! {
    #[test]
    fn test_load_cell_length(ref tx in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_cell_length(tx)?;
    }
}

fn _test_load_cell_partial(data: &[u8], offset: u64) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, offset); // offset
    machine.set_register(A3, 0); // index
    machine.set_register(A4, u64::from(Source::Transaction(SourceEntry::Input))); // source: 1 input
    machine.set_register(A7, LOAD_CELL_SYSCALL_NUMBER); // syscall number

    let output_cell_data = Bytes::from(data.to_owned());
    let output = build_cell_meta(100, output_cell_data);

    let input_cell_data: Bytes = data.iter().rev().cloned().collect();
    let input_cell = build_cell_meta(100, input_cell_data);
    let outputs = vec![output];
    let resolved_inputs = vec![input_cell.clone()];
    let resolved_cell_deps = vec![];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let data_loader = new_mock_data_loader();
    let mut load_cell = LoadCell::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
    );

    let input_correct_data = input_cell.cell_output.as_slice();

    prop_assert!(machine
        .memory_mut()
        .store64(&size_addr, &(input_correct_data.len() as u64))
        .is_ok());

    prop_assert!(load_cell.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    for (i, addr) in
        (addr..addr + (input_correct_data.len() as u64).saturating_sub(offset)).enumerate()
    {
        prop_assert_eq!(
            machine.memory_mut().load8(&addr),
            Ok(u64::from(input_correct_data[i + offset as usize]))
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn test_load_cell_partial(ref data in any_with::<Vec<u8>>(size_range(1000).lift()), offset in 0u64..2000) {
        _test_load_cell_partial(data, offset)?;
    }
}

fn _test_load_cell_capacity(capacity: Capacity) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A3, 0); //index
    machine.set_register(A4, u64::from(Source::Transaction(SourceEntry::Input))); //source: 1 input
    machine.set_register(A5, CellField::Capacity as u64); //field: 0 capacity
    machine.set_register(A7, LOAD_CELL_BY_FIELD_SYSCALL_NUMBER); // syscall number

    let data = Bytes::new();
    let data_hash = CellOutput::calc_data_hash(&data);
    let input_cell = CellMeta {
        out_point: OutPoint::default(),
        transaction_info: None,
        cell_output: CellOutput::new_builder().capacity(capacity.pack()).build(),
        data_bytes: 0,
        mem_cell_data: Some(data),
        mem_cell_data_hash: Some(data_hash),
    };
    let outputs = vec![];
    let resolved_inputs = vec![input_cell];
    let resolved_cell_deps = vec![];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let data_loader = new_mock_data_loader();
    let mut load_cell = LoadCell::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
    );

    prop_assert!(machine.memory_mut().store64(&size_addr, &16).is_ok());

    prop_assert!(load_cell.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(machine.memory_mut().load64(&size_addr), Ok(8));

    let mut buffer = vec![];
    buffer.write_u64::<LittleEndian>(capacity.as_u64()).unwrap();

    for (i, addr) in (addr..addr + buffer.len() as u64).enumerate() {
        prop_assert_eq!(machine.memory_mut().load8(&addr), Ok(u64::from(buffer[i])));
    }
    Ok(())
}

proptest! {
    #[test]
    fn test_load_cell_capacity(capacity in any::<u64>()) {
        _test_load_cell_capacity(Capacity::shannons(capacity))?;
    }
}

#[test]
fn test_load_missing_contract() {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A3, 0); //index
    machine.set_register(A4, u64::from(Source::Transaction(SourceEntry::Output))); //source: 2 output
    machine.set_register(A5, CellField::Type as u64); //field: 4 type
    machine.set_register(A7, LOAD_CELL_BY_FIELD_SYSCALL_NUMBER); // syscall number

    let output_cell = build_cell_meta(100, Bytes::new());
    let outputs = vec![output_cell];
    let resolved_inputs = vec![];
    let resolved_cell_deps = vec![];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let data_loader = new_mock_data_loader();
    let mut load_cell = LoadCell::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
    );

    assert!(machine.memory_mut().store64(&size_addr, &100).is_ok());

    assert!(load_cell.ecall(&mut machine).is_ok());
    assert_eq!(machine.registers()[A0], u64::from(ITEM_MISSING));

    assert_eq!(machine.memory_mut().load64(&size_addr), Ok(100));

    for addr in addr..addr + 100 {
        assert_eq!(machine.memory_mut().load8(&addr), Ok(0));
    }
}

fn _test_load_header(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A3, 0); //index
    machine.set_register(A4, u64::from(Source::Transaction(SourceEntry::HeaderDep))); //source: 4 header
    machine.set_register(A7, LOAD_HEADER_SYSCALL_NUMBER); // syscall number

    let data_hash = blake2b_256(&data).pack();
    let header = HeaderBuilder::default()
        .transactions_root(data_hash)
        .build();

    let header_correct_bytes = header.data();
    let header_correct_data = header_correct_bytes.as_slice();

    let mut headers = HashMap::default();
    headers.insert(header.hash(), header.clone());
    let data_loader = MockDataLoader { headers };
    let header_deps = vec![header.hash()];
    let resolved_inputs = vec![];
    let resolved_cell_deps = vec![];
    let group_inputs = vec![];
    let mut load_header = LoadHeader::new(
        &data_loader,
        header_deps.pack(),
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
    );

    prop_assert!(machine
        .memory_mut()
        .store64(&size_addr, &(header_correct_data.len() as u64 + 20))
        .is_ok());

    prop_assert!(load_header.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(
        machine.memory_mut().load64(&size_addr),
        Ok(header_correct_data.len() as u64)
    );

    for (i, addr) in (addr..addr + header_correct_data.len() as u64).enumerate() {
        prop_assert_eq!(
            machine.memory_mut().load8(&addr),
            Ok(u64::from(header_correct_data[i]))
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn test_load_header(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_header(data)?;
    }
}

fn _test_load_epoch_number(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A3, 0); //index
    machine.set_register(A4, u64::from(Source::Transaction(SourceEntry::HeaderDep))); //source: 4 header
    machine.set_register(A5, HeaderField::EpochNumber as u64);
    machine.set_register(A7, LOAD_HEADER_BY_FIELD_SYSCALL_NUMBER); // syscall number

    let data_hash: H256 = blake2b_256(&data).into();
    let header = HeaderBuilder::default()
        .transactions_root(data_hash.pack())
        .number(2000.pack())
        .epoch(EpochNumberWithFraction::new(1, 40, 1000).pack())
        .build();

    let mut correct_data = [0u8; 8];
    LittleEndian::write_u64(&mut correct_data, 1);

    let mut headers = HashMap::default();
    headers.insert(header.hash(), header.clone());
    let data_loader = MockDataLoader { headers };
    let header_deps = vec![header.hash()];
    let resolved_inputs = vec![];
    let resolved_cell_deps = vec![];
    let group_inputs = vec![];
    let mut load_header = LoadHeader::new(
        &data_loader,
        header_deps.pack(),
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
    );

    prop_assert!(machine
        .memory_mut()
        .store64(&size_addr, &(correct_data.len() as u64 + 20))
        .is_ok());

    prop_assert!(load_header.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(
        machine.memory_mut().load64(&size_addr),
        Ok(correct_data.len() as u64)
    );

    Ok(())
}

proptest! {
    #[test]
    fn test_load_epoch_number(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_epoch_number(data)?;
    }
}

fn _test_load_tx_hash(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A7, LOAD_TX_HASH_SYSCALL_NUMBER); // syscall number

    let transaction_view = TransactionBuilder::default()
        .output_data(data.pack())
        .build();

    let hash = transaction_view.hash();
    let hash_len = 32u64;
    let mut load_tx = LoadTx::new(&transaction_view);

    prop_assert!(machine
        .memory_mut()
        .store64(&size_addr, &(hash_len + 20))
        .is_ok());

    prop_assert!(load_tx.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(machine.memory_mut().load64(&size_addr), Ok(hash_len));

    for (i, addr) in (addr..addr + hash_len as u64).enumerate() {
        prop_assert_eq!(
            machine.memory_mut().load8(&addr),
            Ok(u64::from(hash.as_slice()[i]))
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn test_load_tx_hash(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_tx_hash(data)?;
    }
}

fn _test_load_tx(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A7, LOAD_TRANSACTION_SYSCALL_NUMBER); // syscall number

    let transaction_view = TransactionBuilder::default()
        .output_data(data.pack())
        .build();

    let tx = transaction_view.data();
    let tx_len = transaction_view.data().as_slice().len() as u64;
    let mut load_tx = LoadTx::new(&transaction_view);

    prop_assert!(machine
        .memory_mut()
        .store64(&size_addr, &(tx_len + 20))
        .is_ok());

    prop_assert!(load_tx.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(machine.memory_mut().load64(&size_addr), Ok(tx_len));

    for (i, addr) in (addr..addr + tx_len as u64).enumerate() {
        prop_assert_eq!(
            machine.memory_mut().load8(&addr),
            Ok(u64::from(tx.as_slice()[i]))
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn test_load_tx(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_tx(data)?;
    }
}

fn _test_load_current_script_hash(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A7, LOAD_SCRIPT_HASH_SYSCALL_NUMBER); // syscall number

    let script = Script::new_builder()
        .args(Bytes::from(data.to_owned()).pack())
        .hash_type(ScriptHashType::Data.into())
        .build();
    let hash = script.calc_script_hash();
    let data = hash.raw_data();
    let mut load_script_hash = LoadScriptHash::new(hash);

    prop_assert!(machine.memory_mut().store64(&size_addr, &64).is_ok());

    prop_assert!(load_script_hash.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(
        machine.memory_mut().load64(&size_addr),
        Ok(data.len() as u64)
    );

    for (i, addr) in (addr..addr + data.len() as u64).enumerate() {
        prop_assert_eq!(machine.memory_mut().load8(&addr), Ok(u64::from(data[i])));
    }

    machine.set_register(A0, addr); // addr
    prop_assert!(machine.memory_mut().store64(&size_addr, &0).is_ok());

    prop_assert!(load_script_hash.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(
        machine.memory_mut().load64(&size_addr),
        Ok(data.len() as u64)
    );
    Ok(())
}

proptest! {
    #[test]
    fn test_load_current_script_hash(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_current_script_hash(data)?;
    }
}

fn _test_load_input_lock_script_hash(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A3, 0); //index
    machine.set_register(A4, u64::from(Source::Transaction(SourceEntry::Input))); //source: 1 input
    machine.set_register(A5, CellField::LockHash as u64); //field: 2 lock hash
    machine.set_register(A7, LOAD_CELL_BY_FIELD_SYSCALL_NUMBER); // syscall number

    let script = Script::new_builder()
        .args(Bytes::from(data.to_owned()).pack())
        .hash_type(ScriptHashType::Data.into())
        .build();
    let h = script.calc_script_hash();
    let hash = h.as_bytes();

    let mut input_cell = build_cell_meta(1000, Bytes::new());
    let output_with_lock = input_cell
        .cell_output
        .clone()
        .as_builder()
        .lock(script)
        .build();
    input_cell.cell_output = output_with_lock;
    let outputs = vec![];
    let resolved_inputs = vec![input_cell];
    let resolved_cell_deps = vec![];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let data_loader = new_mock_data_loader();
    let mut load_cell = LoadCell::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
    );

    prop_assert!(machine.memory_mut().store64(&size_addr, &64).is_ok());

    prop_assert!(load_cell.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(
        machine.memory_mut().load64(&size_addr),
        Ok(hash.len() as u64)
    );

    for (i, addr) in (addr..addr + hash.len() as u64).enumerate() {
        prop_assert_eq!(machine.memory_mut().load8(&addr), Ok(u64::from(hash[i])));
    }
    Ok(())
}

proptest! {
    #[test]
    fn test_load_input_lock_script_hash(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_input_lock_script_hash(data)?;
    }
}

fn _test_load_witness(data: &[u8], source: SourceEntry) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A3, 0); //index
    machine.set_register(A4, u64::from(Source::Transaction(source))); //source
    machine.set_register(A7, LOAD_WITNESS_SYSCALL_NUMBER); // syscall number

    let witness = Bytes::from(data.to_owned()).pack();

    let witness_correct_data = witness.raw_data();

    let witnesses = vec![witness];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let mut load_witness = LoadWitness::new(witnesses.pack(), &group_inputs, &group_outputs);

    prop_assert!(machine
        .memory_mut()
        .store64(&size_addr, &(witness_correct_data.len() as u64 + 20))
        .is_ok());

    prop_assert!(load_witness.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(
        machine.memory_mut().load64(&size_addr),
        Ok(witness_correct_data.len() as u64)
    );

    for (i, addr) in (addr..addr + witness_correct_data.len() as u64).enumerate() {
        prop_assert_eq!(
            machine.memory_mut().load8(&addr),
            Ok(u64::from(witness_correct_data[i]))
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn test_load_witness_by_input(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_witness(data, SourceEntry::Input)?;
    }

    #[test]
    fn test_load_witness_by_output(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_witness(data, SourceEntry::Output)?;
    }
}

fn _test_load_group_witness(data: &[u8], source: SourceEntry) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A3, 0); //index
    machine.set_register(A4, u64::from(Source::Group(source))); //source
    machine.set_register(A7, LOAD_WITNESS_SYSCALL_NUMBER); // syscall number

    let witness = Bytes::from(data.to_owned()).pack();

    let witness_correct_data = witness.raw_data();

    let dummy_witness = Bytes::default().pack();
    let witnesses = vec![dummy_witness, witness];
    let group_inputs = vec![1];
    let group_outputs = vec![1];
    let mut load_witness = LoadWitness::new(witnesses.pack(), &group_inputs, &group_outputs);

    prop_assert!(machine
        .memory_mut()
        .store64(&size_addr, &(witness_correct_data.len() as u64 + 20))
        .is_ok());

    prop_assert!(load_witness.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(
        machine.memory_mut().load64(&size_addr),
        Ok(witness_correct_data.len() as u64)
    );

    for (i, addr) in (addr..addr + witness_correct_data.len() as u64).enumerate() {
        prop_assert_eq!(
            machine.memory_mut().load8(&addr),
            Ok(u64::from(witness_correct_data[i]))
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn test_load_group_witness_by_input(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_group_witness(data, SourceEntry::Input)?;
    }

    fn test_load_group_witness_by_output(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_group_witness(data, SourceEntry::Output)?;
    }
}

fn _test_load_script(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 0;
    let addr: u64 = 100;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size_addr
    machine.set_register(A2, 0); // offset
    machine.set_register(A7, LOAD_SCRIPT_SYSCALL_NUMBER); // syscall number

    let script = ScriptBuilder::default()
        .args(Bytes::from(data.to_owned()).pack())
        .build();
    let script_correct_data = script.as_slice();

    let mut load_script = LoadScript::new(script.clone());

    prop_assert!(machine
        .memory_mut()
        .store64(&size_addr, &(script_correct_data.len() as u64 + 20))
        .is_ok());

    prop_assert!(load_script.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    prop_assert_eq!(
        machine.memory_mut().load64(&size_addr),
        Ok(script_correct_data.len() as u64)
    );

    for (i, addr) in (addr..addr + script_correct_data.len() as u64).enumerate() {
        prop_assert_eq!(
            machine.memory_mut().load8(&addr),
            Ok(u64::from(script_correct_data[i]))
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn test_load_script(ref data in any_with::<Vec<u8>>(size_range(1000).lift())) {
        _test_load_script(data)?;
    }
}

fn _test_load_cell_data_as_code(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();

    let addr = 4096;
    let addr_size = 4096;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, addr_size); // size
    machine.set_register(A2, 0); // content offset
    machine.set_register(A3, data.len() as u64); // content size
    machine.set_register(A4, 0); //index
    machine.set_register(A5, u64::from(Source::Transaction(SourceEntry::CellDep))); //source
    machine.set_register(A7, LOAD_CELL_DATA_AS_CODE_SYSCALL_NUMBER); // syscall number

    let dep_cell_data = Bytes::from(data.to_owned());
    let dep_cell = build_cell_meta(10000, dep_cell_data);

    let data_loader = new_mock_data_loader();
    let outputs = vec![];
    let resolved_inputs = vec![];
    let resolved_cell_deps = vec![dep_cell];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let tracing_flags = Default::default();
    let mut load_code = LoadCellData::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
        &tracing_flags,
    );

    prop_assert!(machine.memory_mut().store_byte(addr, addr_size, 1).is_ok());

    prop_assert!(load_code.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    let flags = FLAG_EXECUTABLE | FLAG_FREEZED | FLAG_DIRTY;
    prop_assert_eq!(
        machine
            .memory_mut()
            .fetch_flag(addr / RISCV_PAGESIZE as u64),
        Ok(flags)
    );
    for (i, addr) in (addr..addr + data.len() as u64).enumerate() {
        prop_assert_eq!(machine.memory_mut().load8(&addr), Ok(u64::from(data[i])));
    }
    if (data.len() as u64) < addr_size {
        for i in (data.len() as u64)..addr_size {
            prop_assert_eq!(machine.memory_mut().load8(&(addr + i)), Ok(0));
        }
    }
    Ok(())
}

fn _test_load_cell_data(data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let size_addr: u64 = 100;
    let addr = 4096;
    let addr_size = 4096;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, size_addr); // size
    machine.set_register(A2, 0); // offset
    machine.set_register(A3, 0); //index
    machine.set_register(A4, u64::from(Source::Transaction(SourceEntry::CellDep))); //source
    machine.set_register(A7, LOAD_CELL_DATA_SYSCALL_NUMBER); // syscall number

    prop_assert!(machine.memory_mut().store64(&size_addr, &addr_size).is_ok());

    let dep_cell_data = Bytes::from(data.to_owned());
    let dep_cell = build_cell_meta(10000, dep_cell_data);

    let data_loader = new_mock_data_loader();
    let outputs = vec![];
    let resolved_inputs = vec![];
    let resolved_deps = vec![dep_cell];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let tracing_flags = Default::default();
    let mut load_code = LoadCellData::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_deps,
        &group_inputs,
        &group_outputs,
        &tracing_flags,
    );

    prop_assert!(load_code.ecall(&mut machine).is_ok());
    prop_assert_eq!(machine.registers()[A0], u64::from(SUCCESS));

    let flags = FLAG_WRITABLE | FLAG_DIRTY;
    prop_assert_eq!(
        machine
            .memory_mut()
            .fetch_flag(addr / RISCV_PAGESIZE as u64),
        Ok(flags)
    );
    for (i, addr) in (addr..addr + data.len() as u64).enumerate() {
        prop_assert_eq!(machine.memory_mut().load8(&addr), Ok(u64::from(data[i])));
    }
    if (data.len() as u64) < addr_size {
        for i in (data.len() as u64)..addr_size {
            prop_assert_eq!(machine.memory_mut().load8(&(addr + i)), Ok(0));
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 10, .. ProptestConfig::default()
    })]
    #[test]
    fn test_load_code(ref data in any_with::<Vec<u8>>(size_range(4096).lift())) {
        _test_load_cell_data_as_code(data)?;
    }

    #[test]
    fn test_load_data(ref data in any_with::<Vec<u8>>(size_range(4096).lift())) {
        _test_load_cell_data(data)?;
    }
}

#[test]
fn test_load_overflowed_cell_data_as_code() {
    let data = vec![0, 1, 2, 3, 4, 5];
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let addr = 4096;
    let addr_size = 4096;

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, addr_size); // size
    machine.set_register(A2, 3); // content offset
    machine.set_register(A3, u64::max_value() - 1); // content size
    machine.set_register(A4, 0); //index
    machine.set_register(A5, u64::from(Source::Transaction(SourceEntry::CellDep))); //source
    machine.set_register(A7, LOAD_CELL_DATA_AS_CODE_SYSCALL_NUMBER); // syscall number

    let dep_cell_data = Bytes::from(data);
    let dep_cell = build_cell_meta(10000, dep_cell_data);

    let data_loader = new_mock_data_loader();
    let outputs = vec![];
    let resolved_inputs = vec![];
    let resolved_cell_deps = vec![dep_cell];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let tracing_flags = Default::default();
    let mut load_code = LoadCellData::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
        &tracing_flags,
    );

    assert!(machine.memory_mut().store_byte(addr, addr_size, 1).is_ok());

    let result = load_code.ecall(&mut machine);
    assert_eq!(result.unwrap_err(), VMError::MemOutOfBound);
}

fn _test_load_cell_data_on_freezed_memory(as_code: bool, data: &[u8]) -> Result<(), TestCaseError> {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let addr = 8192;
    let addr_size = 4096;

    prop_assert!(machine
        .memory_mut()
        .init_pages(addr, addr_size, FLAG_EXECUTABLE | FLAG_FREEZED, None, 0)
        .is_ok());

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, addr_size); // size
    machine.set_register(A2, 0); // content offset
    machine.set_register(A3, data.len() as u64); // content size
    machine.set_register(A4, 0); //index
    machine.set_register(A5, u64::from(Source::Transaction(SourceEntry::CellDep))); //source
    let syscall = if as_code {
        LOAD_CELL_DATA_AS_CODE_SYSCALL_NUMBER
    } else {
        LOAD_CELL_DATA_SYSCALL_NUMBER
    };
    machine.set_register(A7, syscall); // syscall number

    let dep_cell_data = Bytes::from(data.to_owned());
    let dep_cell = build_cell_meta(10000, dep_cell_data);

    let data_loader = new_mock_data_loader();
    let outputs = vec![];
    let resolved_inputs = vec![];
    let resolved_cell_deps = vec![dep_cell];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let tracing_flags = Default::default();
    let mut load_code = LoadCellData::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
        &tracing_flags,
    );

    prop_assert!(load_code.ecall(&mut machine).is_err());

    for i in addr..addr + addr_size {
        assert_eq!(machine.memory_mut().load8(&i), Ok(0));
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 10, .. ProptestConfig::default()
    })]
    #[test]
    fn test_load_code_on_freezed_memory(ref data in any_with::<Vec<u8>>(size_range(4096).lift())) {
        _test_load_cell_data_on_freezed_memory(true, data)?;
    }

    #[test]
    fn test_load_data_on_freezed_memory(ref data in any_with::<Vec<u8>>(size_range(4096).lift())) {
        _test_load_cell_data_on_freezed_memory(false, data)?;
    }
}

#[test]
fn test_load_code_unaligned_error() {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let addr = 4097;
    let addr_size = 4096;
    let data = [2; 32];

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, addr_size); // size
    machine.set_register(A2, 0); // content offset
    machine.set_register(A3, data.len() as u64); // content size
    machine.set_register(A4, 0); //index
    machine.set_register(A5, u64::from(Source::Transaction(SourceEntry::CellDep))); //source
    machine.set_register(A7, LOAD_CELL_DATA_AS_CODE_SYSCALL_NUMBER); // syscall number
    let dep_cell_data = Bytes::from(data.to_vec());
    let dep_cell = build_cell_meta(10000, dep_cell_data);

    let data_loader = new_mock_data_loader();
    let outputs = vec![];
    let resolved_inputs = vec![];
    let resolved_cell_deps = vec![dep_cell];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let tracing_flags = Default::default();
    let mut load_code = LoadCellData::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
        &tracing_flags,
    );

    assert!(machine.memory_mut().store_byte(addr, addr_size, 1).is_ok());

    assert!(load_code.ecall(&mut machine).is_err());

    for i in addr..addr + addr_size {
        assert_eq!(machine.memory_mut().load8(&i), Ok(1));
    }
}

#[test]
fn test_load_code_slice_out_of_bound_error() {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let addr = 4096;
    let addr_size = 4096;
    let data = [2; 32];

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, addr_size); // size
    machine.set_register(A2, 0); // content offset
    machine.set_register(A3, data.len() as u64 + 3); // content size
    machine.set_register(A4, 0); //index
    machine.set_register(A5, u64::from(Source::Transaction(SourceEntry::CellDep))); //source
    machine.set_register(A7, LOAD_CELL_DATA_AS_CODE_SYSCALL_NUMBER); // syscall number

    let dep_cell_data = Bytes::from(data.to_vec());
    let dep_cell = build_cell_meta(10000, dep_cell_data);

    let data_loader = new_mock_data_loader();
    let outputs = vec![];
    let resolved_inputs = vec![];
    let resolved_cell_deps = vec![dep_cell];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let tracing_flags = Default::default();
    let mut load_code = LoadCellData::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
        &tracing_flags,
    );

    assert!(machine.memory_mut().store_byte(addr, addr_size, 1).is_ok());

    assert!(load_code.ecall(&mut machine).is_ok());
    assert_eq!(machine.registers()[A0], u64::from(SLICE_OUT_OF_BOUND));

    for i in addr..addr + addr_size {
        assert_eq!(machine.memory_mut().load8(&i), Ok(1));
    }
}

#[test]
fn test_load_code_not_enough_space_error() {
    let mut machine = SCRIPT_VERSION.init_core_machine_without_limit();
    let addr = 4096;
    let addr_size = 4096;

    let mut data = vec![];
    data.resize(8000, 2);

    machine.set_register(A0, addr); // addr
    machine.set_register(A1, addr_size); // size
    machine.set_register(A2, 0); // content offset
    machine.set_register(A3, data.len() as u64); // content size
    machine.set_register(A4, 0); //index
    machine.set_register(A5, u64::from(Source::Transaction(SourceEntry::CellDep))); //source
    machine.set_register(A7, LOAD_CELL_DATA_AS_CODE_SYSCALL_NUMBER); // syscall number

    let dep_cell_data = Bytes::from(data);
    let dep_cell = build_cell_meta(10000, dep_cell_data);

    let data_loader = new_mock_data_loader();
    let outputs = vec![];
    let resolved_inputs = vec![];
    let resolved_cell_deps = vec![dep_cell];
    let group_inputs = vec![];
    let group_outputs = vec![];
    let tracing_flags = Default::default();
    let mut load_code = LoadCellData::new(
        &data_loader,
        &outputs,
        &resolved_inputs,
        &resolved_cell_deps,
        &group_inputs,
        &group_outputs,
        &tracing_flags,
    );

    assert!(machine.memory_mut().store_byte(addr, addr_size, 1).is_ok());

    assert!(load_code.ecall(&mut machine).is_ok());
    assert_eq!(machine.registers()[A0], u64::from(SLICE_OUT_OF_BOUND));

    for i in addr..addr + addr_size {
        assert_eq!(machine.memory_mut().load8(&i), Ok(1));
    }
}
