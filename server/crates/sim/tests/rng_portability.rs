//! Identical known answers on native targets and wasm32 (run with wasm-bindgen-test-runner).
use rand::{seq::SliceRandom, Rng, RngCore, SeedableRng};
use rts_sim::rng::PortableRng;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn reference_xoshiro_state_words() {
    let mut seed = [0; 32];
    for (chunk, word) in seed.chunks_exact_mut(8).zip(1u64..=4) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    let mut rng = PortableRng::from_seed(seed);
    // Published Xoshiro256++ reference sequence for state [1, 2, 3, 4].
    for expected in [
        41943041,
        58720359,
        3588806011781223,
        3591011842654386,
        9228616714210784205,
        9973669472204895162,
        14011001112246962877,
        12406186145184390807,
        15849039046786891736,
        10450023813501588000,
    ] {
        assert_eq!(rng.next_u64(), expected);
    }
}

#[cfg(target_pointer_width = "64")]
#[test]
fn preserves_historical_native_seed_and_sampling() {
    use rand::rngs::SmallRng;
    for seed in [0, 1, 0x12345678, u64::MAX] {
        let mut portable = PortableRng::seed_from_u64(seed);
        let mut historical = SmallRng::seed_from_u64(seed);
        for len in 0..32 {
            assert_eq!(portable.next_u32(), historical.next_u32());
            assert_eq!(portable.next_u64(), historical.next_u64());
            let mut a = vec![0; len];
            let mut b = a.clone();
            portable.fill_bytes(&mut a);
            historical.fill_bytes(&mut b);
            assert_eq!(a, b);
            assert_eq!(portable.gen::<f32>(), historical.gen::<f32>());
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn seeded_known_answers() {
    let mut rng = PortableRng::seed_from_u64(0);
    for expected in [
        8251690495967107212,
        8100708189767581495,
        18075600217600495122,
        8525480561105331059,
        16548187137891372853,
        17394354212723026293,
        10849406535690575840,
        8418576879742075557,
    ] {
        assert_eq!(rng.next_u64(), expected);
    }
    let mut rng = PortableRng::seed_from_u64(1);
    for expected in [
        13159342511175687856,
        711931187601865195,
        3354571192391781809,
        6333577224427989655,
        8779277544145138948,
        17679115517432482121,
        2049633066944016414,
        7584348827180842295,
    ] {
        assert_eq!(rng.next_u64(), expected);
    }
    let mut rng = PortableRng::seed_from_u64(305419896);
    for expected in [
        2283429341148430538,
        14553925039101821570,
        8391521534902208855,
        3292210788918423609,
        13524289724607009377,
        3417943900055850780,
        567495677543639757,
        15764138414281557387,
    ] {
        assert_eq!(rng.next_u64(), expected);
    }
    let mut rng = PortableRng::seed_from_u64(18446744073709551615);
    for expected in [
        11345198270385851335,
        2159648510999386821,
        15539204399642874415,
        6138335230395853604,
        16072506671903263049,
        9589522811998761760,
        2462082557297450375,
        11344595678325010327,
    ] {
        assert_eq!(rng.next_u64(), expected);
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn sampling_known_answers() {
    let mut rng = PortableRng::seed_from_u64(0x12345678);
    let floats: Vec<_> = (0..4).map(|_| rng.gen::<f32>().to_bits()).collect();
    let spreads: Vec<_> = (0..4)
        .map(|_| rng.gen_range(-0.25f32..=0.25).to_bits())
        .collect();
    let tickets: Vec<_> = (0..4).map(|_| rng.gen_range(0u64..100)).collect();
    let mut starts = [0, 1, 2, 3, 4, 5, 6, 7];
    starts.shuffle(&mut rng);
    assert_eq!(floats, [1040024304, 1061812717, 1055451480, 1043775776]);
    assert_eq!(spreads, [1039056820, 3189842448, 3195027380, 1043696372]);
    assert_eq!(tickets, [75, 74, 59, 30]);
    assert_eq!(starts, [1, 0, 4, 5, 6, 7, 2, 3]);
}

/// Full committed-state projection, including floats, unordered-store adapters, map and RNG.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn end_state_fingerprint_matches_native_and_wasm() {
    use rts_sim::game::{Game, PlayerInit};
    let mut game = Game::new(
        &[PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: rts_protocol::DEFAULT_FACTION_ID.to_string(),
            name: "Hash fixture".into(),
            color: "#ffffff".into(),
            is_ai: false,
        }],
        42,
    );
    for _ in 0..3 {
        game.tick();
    }
    assert_eq!(
        game.replay_end_state_hash().unwrap(),
        "0ae0d4f6f671c83ae7ca6c5a437cbadd23690f174e792d2bd734b9889f8ea6d1"
    );
}
