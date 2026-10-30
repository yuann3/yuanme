cat > content/posts/odd_sum.rs <<'RS'
fn odd_sum(n: u64) -> u64 {
    (1..=n).map(|k| 2 * k - 1).sum()
}
RS
perl -0pi -e 's/#\+begin_src rust\n.*?#\+end_src\n/#+INCLUDE: "odd_sum.rs" src rust\n/s' content/posts/hello-org.org
grep -q '^#+INCLUDE' content/posts/hello-org.org
