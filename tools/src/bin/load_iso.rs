fn main() {
    let x = isoparser::load_iso(r"C:\Users\Host\Downloads\ISOTemplate\ISOTemplate Viewer\fing.ist")
        .unwrap();
    dbg!(x);
}
