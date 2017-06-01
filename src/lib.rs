struct Context {
    x: u16,
}

struct Bar {
    cx: Context,
}

struct Baz {
    cx: Context,
}


enum State {
    Bar(Bar),
    Baz(Baz),
}


impl Baz {
    fn process_stuff(self) -> State {
        // do some stuff, note that the state
        // could be anything here
        State::Bar(Bar{ cx: self.cx })
    }
}


#[test]
fn it_works() {
}
