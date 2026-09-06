use nameless::challenge::TestCase;
use std::time::Duration;

#[tokio::test]
async fn reference_passes_tests() {
    let tests = vec![
        TestCase {
            input: "&[1, 2, 3, 4, 5]".into(),
            expected: "15".into(),
        },
        TestCase {
            input: "&[-1, 0, 1]".into(),
            expected: "0".into(),
        },
        TestCase {
            input: "&[]".into(),
            expected: "0".into(),
        },
    ];
    let func = "fn solve(input: &[i64]) -> i64 {\n    input.iter().sum()\n}";
    let result = nameless::harness::evaluate_candidate(
        func,
        &tests,
        Duration::from_secs(30),
        Duration::from_secs(5),
    )
    .await
    .unwrap();

    assert!(result.passed, "reference should pass: {}", result.error);
    assert!(result.time_ns > 0, "should measure time");
    assert!(result.binary_size > 0, "should have binary size");
    assert!(result.compile_time_ns > 0, "should have compile time");
}

#[tokio::test]
async fn bad_candidate_fails_tests() {
    let tests = vec![TestCase {
        input: "&[1, 2, 3]".into(),
        expected: "6".into(),
    }];
    let func = "fn solve(input: &[i64]) -> i64 {\n    42\n}";
    let result = nameless::harness::evaluate_candidate(
        func,
        &tests,
        Duration::from_secs(30),
        Duration::from_secs(5),
    )
    .await
    .unwrap();

    assert!(!result.passed, "wrong answer should fail");
    assert!(!result.error.is_empty(), "should have error message");
}

#[tokio::test]
async fn invalid_rust_fails_compilation() {
    let tests = vec![TestCase {
        input: "&[1]".into(),
        expected: "1".into(),
    }];
    let func = "fn solve(input: &[i64]) -> i64 {\n    this is not valid rust\n}";
    let result = nameless::harness::evaluate_candidate(
        func,
        &tests,
        Duration::from_secs(30),
        Duration::from_secs(5),
    )
    .await
    .unwrap();

    assert!(!result.passed, "invalid code should fail");
    assert!(
        result.error.contains("compilation failed"),
        "should have compile error: {}",
        result.error
    );
}

#[tokio::test]
async fn scorer_better_than_reference() {
    // Same result as reference → score ≈ 1.0
    let reference = nameless::harness::HarnessResult {
        passed: true,
        error: String::new(),
        time_ns: 100,
        memory_bytes: 1000,
        binary_size: 5000,
        compile_time_ns: 500000,
    };
    let candidate = nameless::harness::HarnessResult {
        passed: true,
        error: String::new(),
        time_ns: 50,
        memory_bytes: 500,
        binary_size: 4000,
        compile_time_ns: 400000,
    };
    let score = nameless::scorer::score(&candidate, &reference);
    assert!(
        score > 1.0,
        "faster candidate should score > 1.0, got {score}"
    );

    // Failed candidate → 0.0
    let failed = nameless::harness::HarnessResult {
        passed: false,
        error: "test failed".into(),
        time_ns: 10,
        memory_bytes: 0,
        binary_size: 4000,
        compile_time_ns: 400000,
    };
    let score = nameless::scorer::score(&failed, &reference);
    assert_eq!(score, 0.0, "failed candidate should score 0.0");
}

#[tokio::test]
async fn db_roundtrip() {
    let pool = nameless::db::init_pool(":memory:").await.unwrap();

    let challenge = nameless::challenge::Challenge {
        name: "test_sum".into(),
        description: "sum array".into(),
        signature: "fn solve(input: &[i64]) -> i64".into(),
        config: nameless::challenge::ChallengeConfig {
            max_generations: 5,
            candidates_per_generation: 2,
            timeout_compile_secs: 30,
            timeout_run_secs: 5,
        },
        tests: vec![nameless::challenge::TestCase {
            input: "&[1,2,3]".into(),
            expected: "6".into(),
        }],
    };

    let id = nameless::db::upsert_challenge(&pool, &challenge)
        .await
        .unwrap();
    assert!(!id.is_empty());

    // Upsert again returns same id
    let id2 = nameless::db::upsert_challenge(&pool, &challenge)
        .await
        .unwrap();
    assert_eq!(id, id2);

    // Insert candidate + result
    let cid = nameless::db::insert_candidate(
        &pool,
        &id,
        1,
        "fresh",
        "fn solve(x: &[i64]) -> i64 { x.iter().sum() }",
    )
    .await
    .unwrap();
    let result = nameless::harness::HarnessResult {
        passed: true,
        error: String::new(),
        time_ns: 50,
        memory_bytes: 0,
        binary_size: 5000,
        compile_time_ns: 500000,
    };
    nameless::db::insert_result(&pool, &cid, &result, 1.23)
        .await
        .unwrap();

    // Query best
    let best = nameless::db::get_best_candidates(&pool, &id, 10)
        .await
        .unwrap();
    assert_eq!(best.len(), 1);
    assert_eq!(best[0].score, 1.23);

    // List challenges
    let list = nameless::db::list_challenges_with_scores(&pool)
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "test_sum");
    assert_eq!(list[0].best_score, 1.23);

    // Leaderboard
    let lb = nameless::db::get_leaderboard(&pool, "test_sum")
        .await
        .unwrap();
    assert_eq!(lb.len(), 1);
    assert_eq!(lb[0].rank, 1);
}
