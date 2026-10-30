perl -0pi -e 's/#\+INCLUDE: "odd_sum.rs" src rust\n/#+begin_src rust\nfn odd_sum(n: u64) -> u64 {\n    (1..=n).map(|k| 2 * k - 1).sum()\n}\n#+end_src\n/' content/posts/hello-org.org
