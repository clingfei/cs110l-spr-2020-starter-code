1.**“One thing that’s confusing is why sometimes I need to &var and other  times I can just use var: for example, set.contains(&var), but  set.insert(var) – why?**

*mutable reference: 可以通过reference来修改指向的值*

Rust doesn’t allow multiple references to exist when a mutable reference has been borrowed.

```Rust
fn main() {
    let mut s = String::from("hello");
    let s1 = &mut s;
    let s2 = &s;     // not allowed，because s1 is a mutable borrowed from s.
    println!("{} {} {}", s, s1, s2);  // s1 borrowed from s, and haven't return to s, so s is not usable.
}
```

while following codes really works:

```rust
fn main() {
    let mut s = String::from("hello");
    let s1 = &mut s;
    println!("{}", s1);
    println!("{}", s)
}
```

When inserting an item into a set, we want to **transfer ownership of that item into the set**; that way, the item **will exist as long as the set exists**. (It would be bad if you added a string to the set, and then someone freed the string while it was still a member of the set.) However, when trying to see if the set contains an item, we want to retain ownership, so we only pass a reference.

2.Ownership writeup

（1） cannot compile, because s is borrowed by ref1, therefore cannot assign to s

```rust
fn main() {
    let mut s = String::from("hello");
    let ref1 = &s;
    let ref2 = &ref1;
    let ref3 = &ref2;
    s = String::from("goodbye");
    println!("{}", ref3.to_uppercase());
}
```

(2) cannot compile, because this function returns type contains a borrowed value, while there is not a lifetime specifier.

```rust
fn drip_drop() -> &String {
    let s = String::from("hello world!");
    return &s;
}
```

(3) cannot compile, v[0]  belongs to v and String don't have default Copy triat

```rust
fn main() {
    let s1 = String::from("hello");
    let mut v = Vec::new();
    v.push(s1);
    let s2: String = v[0];
    println!("{}", s2);
}
```

