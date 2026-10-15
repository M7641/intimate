//! Browser ABI for the engine. No game logic lives here.
//!
//! Every call writes its result into one output buffer: JSON on success, the
//! panic or error message on failure. The host reads it back through
//! `out_ptr` and `out_len`. The buffer is valid until the next call.
//!
//! Input goes the other way through `alloc`: the host reserves `len` bytes,
//! writes into them, and passes the pointer to a call that takes ownership.

use atlantis_core::hero::hero_sheets;
use atlantis_core::sim::{Policy, Rng};
use atlantis_core::{Action, Catalog, Game};
use std::cell::RefCell;

thread_local! {
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static GAME: RefCell<Option<Game>> = const { RefCell::new(None) };
    static RNG: RefCell<Rng> = RefCell::new(Rng::new(42));
}

fn emit(bytes: Vec<u8>) {
    OUT.with(|out| *out.borrow_mut() = bytes);
}

fn emit_snapshot() {
    GAME.with(|slot| {
        let game = slot.borrow();
        let game = game.as_ref().expect("game_new was called");
        emit(serde_json::to_vec(&game.snapshot()).expect("snapshot serialises"));
    });
}

/// Call once after instantiation. Routes panic messages into the buffer.
#[unsafe(no_mangle)]
pub extern "C" fn init() {
    std::panic::set_hook(Box::new(|info| emit(info.to_string().into_bytes())));
}

#[unsafe(no_mangle)]
pub extern "C" fn out_ptr() -> *const u8 {
    OUT.with(|out| out.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn out_len() -> usize {
    OUT.with(|out| out.borrow().len())
}

/// Reserve `len` bytes for the host to write into. Ownership passes to the
/// call that receives the pointer.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len.max(1));
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Every hero's profile and deck, for the card browser. Needs no game.
#[unsafe(no_mangle)]
pub extern "C" fn catalog() {
    let sheets = hero_sheets(&Catalog::load());
    emit(serde_json::to_vec(&sheets).expect("catalogue serialises"));
}

/// Start the teaching game with this seed. Output: the snapshot.
#[unsafe(no_mangle)]
pub extern "C" fn game_new(seed: u64) {
    let mut setup = atlantis_core::Setup::teaching_game();
    setup.seed = seed;
    let game = Game::new(&setup).expect("teaching setup is valid");
    GAME.with(|slot| *slot.borrow_mut() = Some(game));
    RNG.with(|r| *r.borrow_mut() = Rng::new(seed ^ 0x9E37_79B9));
    emit_snapshot();
}

/// Apply one action given as JSON. Returns 0 and the snapshot, or 1 and an
/// error message.
///
/// # Safety
/// `ptr` must come from [`alloc`] with the same `len`, and is consumed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn game_apply(ptr: *mut u8, len: usize) -> u32 {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) };
    let action: Action = match serde_json::from_slice(&bytes) {
        Ok(a) => a,
        Err(e) => {
            emit(format!("bad action JSON: {e}").into_bytes());
            return 1;
        }
    };
    let result = GAME.with(|slot| {
        let mut game = slot.borrow_mut();
        game.as_mut().expect("game_new was called").apply(action)
    });
    match result {
        Ok(()) => {
            emit_snapshot();
            0
        }
        Err(e) => {
            emit(e.to_string().into_bytes());
            1
        }
    }
}

/// Let a bot take one action: 0 random, 1 brawler. Output: the snapshot.
#[unsafe(no_mangle)]
pub extern "C" fn game_step(policy: u32) {
    let policy = if policy == 0 {
        Policy::Random
    } else {
        Policy::Brawler
    };
    GAME.with(|slot| {
        let mut game = slot.borrow_mut();
        let game = game.as_mut().expect("game_new was called");
        RNG.with(|r| {
            game.play_out(policy, &mut r.borrow_mut(), 1);
        });
    });
    emit_snapshot();
}
