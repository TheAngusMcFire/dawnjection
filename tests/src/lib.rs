

#[derive(Debug)]
struct SomeStruct {}
struct IncomingMessage<'r> {msg: &'r str}

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

#[dawnjection_codegen::handler_with_di]
fn some_handler(imsg: IncomingMessage, msg: String, cnt: u32, stru: &SomeStruct) -> Result<(), dawnjection::Report> {
    println!("{}  {}  {}  {:?}", imsg.msg, msg, cnt, stru);
    Ok(())
}

#[tokio::test]
async fn handler_with_di_test() {
    let hndlr = some_handler::default().consumer_entry();
    let sp = dawnjection::ServiceCollection::default()
        .reg_cloneable(format!("test"))
        .reg_cloneable(5_u32)
        .reg_singleton(SomeStruct{})
        .build_service_provider_arc();

    let scope = sp.create_scope_arc(Some(
        
        dawnjection::ServiceCollection::default()
        .reg_takeable(IncomingMessage { msg: "this is a test" })));

    let ret = (hndlr.handler)(scope);
    assert!(ret.await.is_ok())
}
