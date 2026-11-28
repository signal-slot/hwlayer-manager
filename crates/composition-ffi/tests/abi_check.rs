//! Host test of `ce_abi_check` through the crate's own public items.
//!
//! `ce_abi_check` takes the caller's sizes of six structures and the engine
//! storage size and returns a `bool`: there is no version number and no error
//! code in this ABI, so "refused" means `false`. `tests/malformed.rs` already
//! covers the check with C-layout mirrors (accepts the mirror sizes, rejects
//! each wrong slot); this file adds what it lacks: the sizes of the crate's
//! real structures, zero and off-by-one sizes, and the header text agreeing
//! with the Rust constants (`CE_ENGINE_SIZE`, `CE_ENGINE_ALIGN`, the argument
//! order of `CE_ABI_CHECK()`).

use core::mem::{align_of, size_of};

use composition_ffi::*;

const HEADER: &str = include_str!("../include/composition_engine.h");

fn abi(sizes: [usize; 7]) -> bool {
    ce_abi_check(
        sizes[0], sizes[1], sizes[2], sizes[3], sizes[4], sizes[5], sizes[6],
    )
}

fn good() -> [usize; 7] {
    [
        size_of::<ce_capabilities>(),
        size_of::<ce_layer>(),
        size_of::<ce_frame>(),
        size_of::<ce_event>(),
        size_of::<ce_backend_ops>(),
        size_of::<ce_assignment>(),
        CE_ENGINE_SIZE,
    ]
}

fn header_define(name: &str) -> usize {
    HEADER
        .lines()
        .find_map(|line| {
            let mut words = line.split_whitespace();
            (words.next() == Some("#define") && words.next() == Some(name))
                .then(|| words.next().expect("value").parse().expect("number"))
        })
        .unwrap_or_else(|| panic!("{name} is not defined in the header"))
}

#[test]
fn matching_sizes_are_accepted() {
    assert!(abi(good()));
}

#[test]
fn any_wrong_size_is_refused() {
    for slot in 0..7 {
        for wrong in [0, good()[slot] - 1, good()[slot] + 1, usize::MAX] {
            let mut sizes = good();
            sizes[slot] = wrong;
            assert!(!abi(sizes), "slot {slot} size {wrong} was accepted");
        }
    }
}

#[test]
fn all_zero_sizes_are_refused() {
    assert!(!abi([0; 7]));
}

#[test]
fn swapped_arguments_are_refused() {
    // The check is positional: the right values in another order fail.
    let mut sizes = good();
    sizes.swap(0, 6);
    assert!(!abi(sizes));
}

#[test]
fn header_constants_equal_the_rust_constants() {
    assert_eq!(header_define("CE_ENGINE_SIZE"), CE_ENGINE_SIZE);
    assert_eq!(header_define("CE_ENGINE_ALIGN"), CE_ENGINE_ALIGN);
    assert!(align_of::<ce_layer>() <= CE_ENGINE_ALIGN);
}

#[test]
fn header_macro_passes_the_arguments_in_the_functions_order() {
    let text = HEADER.replace("\\\n", " ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let declaration = "bool ce_abi_check(size_t capabilities, size_t layer, size_t frame, \
         size_t event, size_t backend_ops, size_t assignment, size_t engine_size);";
    assert!(text.contains(declaration), "declaration changed");
    let call = "ce_abi_check(sizeof(ce_capabilities), sizeof(ce_layer), sizeof(ce_frame), \
         sizeof(ce_event), sizeof(ce_backend_ops), sizeof(ce_assignment), CE_ENGINE_SIZE)";
    assert!(text.contains(call), "CE_ABI_CHECK() argument order changed");
}
