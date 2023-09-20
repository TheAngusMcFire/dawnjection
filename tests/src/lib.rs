
struct SomeStruct {

}

use dawnjection_codegen::with_di;


#[with_di]
fn some_function(msg: String, cnt: u32, stru: &SomeStruct) {

}


use dawnjection::ServiceCollection;

#[test]
fn di_codegen_int_tes () {
    let mut sc = dawnjection::ServiceCollection::default();
    //sc.
}
