// SPDX-License-Identifier: MIT OR Apache-2.0
//

fn main() {
    // Extra cfgs
    println!("cargo::rustc-check-cfg=cfg(test_in_svsm)");

    // uapi-tester
    println!("cargo::rustc-link-arg-bin=uapi-tester=-no-pie");
    println!("cargo::rustc-link-arg-bin=uapi-tester=-nostdlib");

    // Extra linker args for tests.
    println!("cargo::rerun-if-env-changed=LINK_TEST");
    if std::env::var("LINK_TEST").is_ok() {
        println!("cargo::rustc-cfg=test_in_svsm");
        println!("cargo::rustc-link-arg=-nostdlib");
        println!("cargo::rustc-link-arg=-no-pie");
    }

    println!("cargo::rerun-if-changed=build.rs");
}
