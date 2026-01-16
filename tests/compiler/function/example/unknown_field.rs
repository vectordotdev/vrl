use vrl::example;

fn main() {
    let _ex = example! {
        title: "test",
        source: "code",
        result: Ok("output"),
        unknown_field: "value",
    };
}
