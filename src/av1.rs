mod obu;

#[derive(FromPrimitive, ToPrimitive)]
enum Av1Profile {
    Main = 0,
    High = 1,
    Professional = 2,
}
