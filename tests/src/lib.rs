#[derive(Debug)]
struct SomeStruct {}

#[dawnjection_codegen::with_di]
fn some_function(msg: String, cnt: u32, stru: &SomeStruct) {
    println!("{}  {}  {:?}", msg, cnt, stru)
}

#[test]
fn di_codegen_int_tes() {
    use dawnjection::ServiceCollection;

    let pro = ServiceCollection::default()
        .reg_cloneable("this is a test".to_string())
        .reg_singleton(SomeStruct {})
        .reg_takeable(123)
        .build_service_provider_arc();

    let sc = ServiceCollection::default().reg_takeable(123u32);
    some_function_di(&pro.create_scope(Some(sc)));
}
