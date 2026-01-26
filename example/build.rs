use justcxx_build;

fn main() {
    justcxx_build::bridge("src/lib.rs")
        .file("src/cpp/test.hh")
        .include("src/cpp")      
        .std("c++17")            
        .compile("example"); 
    
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/cpp/test.hh");
}