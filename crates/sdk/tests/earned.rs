use crabtalk_sdk::account::billing::Earned;

#[test]
fn net_is_revenue_less_cut_and_compute() {
    let earned = Earned::new(10, 20_000, 2_000, 1_500);
    assert_eq!(earned.net, 16_500);
}

#[test]
fn net_goes_below_zero() {
    assert_eq!(Earned::new(1, 100, 10, 500).net, -410);
}

#[test]
fn sums_keep_net() {
    let mut month = Earned::default();
    month += Earned::new(3, 600, 60, 100);
    month += Earned::new(0, 0, 0, 40);
    assert_eq!(month, Earned::new(3, 600, 60, 140));
    assert_eq!(month.net, 400);
}
