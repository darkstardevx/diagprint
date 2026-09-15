use diagprint::SourceCache;

const SOURCE_NAME: &str = "memory://editor/revision-demo.rs";

fn main() {
    let cache = SourceCache::new();

    let first = cache.insert_revisioned(SOURCE_NAME, "let answer = old_value();\n");

    let snapshot = cache.snapshot();

    println!("diagnostic revision: {first}");

    let second = cache.insert_revisioned(SOURCE_NAME, "let answer = new_value();\n");

    println!("live revision:       {second}");

    println!(
        "snapshot current:     {}",
        snapshot.is_current(&cache, SOURCE_NAME)
    );

    println!(
        "snapshot stale:       {}",
        snapshot.is_stale(&cache, SOURCE_NAME)
    );

    println!(
        "snapshot source:      {}",
        snapshot
            .get(SOURCE_NAME)
            .expect("snapshot source missing")
            .trim()
    );

    println!(
        "live source:          {}",
        cache.get(SOURCE_NAME).expect("live source missing").trim()
    );
}
