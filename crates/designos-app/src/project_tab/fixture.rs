//! Test fixture: the "Checkout" project's docs, the sample world of the Project tab mockups.

use super::ops::OVERVIEW_DESCRIPTION;
use designos_docs::{DocsStore, Expect, Group, Manifest, Page, Pin};

pub const OVERVIEW: &str = "overview.md";
pub const PAYMENT_METHODS: &str = "spec/payment-methods.md";
pub const PAYMENT_FLOW: &str = "spec/payment-flow.md";
pub const CART_RULES: &str = "spec/cart-rules.md";
pub const ERROR_HANDLING: &str = "spec/error-handling.md";
pub const RESEARCH: &str = "discovery/checkout-research.md";
pub const COMPETITORS: &str = "discovery/competitor-flows.md";

const PAYMENT_METHODS_MD: &str = "---\ntitle: Payment methods\ndescription: How customers pay at checkout, and what happens when a payment fails.\n---\n\n## Accepted methods\n\nCustomers can pay by card, Apple Pay or bank transfer. Methods appear in the order below.\n\n> [!note]\n> Wallets other than Apple Pay are not part of this release.\n\n## Fees and limits\n\n| Method | Fee | Settlement | Refund window | Limit | Notes |\n| --- | --- | --- | --- | --- | --- |\n| Card | 2.9% + $0.30 | 2 days | 120 days | $10,000 | 3-D Secure is required, a declined card shows an inline error |\n| Apple Pay | 2.9% + $0.30 | 2 days | 120 days | $5,000 | Safari and iOS only |\n| Bank transfer | 0.8%, cap $5 | 5 days | 60 days | $50,000 | Manual reconciliation |\n\n## Before release\n\n- [x] Card form validates number, expiry and CVC\n- [x] Declined card shows an inline error\n- [ ] Bank transfer shows the reference number\n\n### Open questions\n\nWhich settlement window does finance want for **bank transfer**? See [Cart rules](cart-rules.md) and `limit`.\n";

/// Writes the Checkout docs: seven pages, three pinned screens and two states. `file_id` is the design file the pins and the embed point at.
pub fn write_checkout_docs(store: &DocsStore, project_name: &str, file_id: &str) {
    let page = |text: &str| Page::parse(text);
    let write = |rel: &str, text: &str| {
        store.write_page(rel, &page(text), Expect::Absent).unwrap();
    };
    write(OVERVIEW, &format!("---\ntitle: {project_name}\ndescription: {OVERVIEW_DESCRIPTION}\n---\n\n## Goals\n\nLet customers review their cart, pay, and see a clear result in under a minute.\n\n## Where things live\n\n- Spec pages describe each payment method\n- Screens and States pin frames from the design files\n- Discovery holds research notes\n\n### Out of scope\n\nRefunds and subscriptions are not part of this release.\n"));
    write(PAYMENT_METHODS, &format!("{PAYMENT_METHODS_MD}\n![Payment](designos:{file_id}#node-payment)\n"));
    write(
        PAYMENT_FLOW,
        "---\ntitle: Payment flow\ndescription: The steps from cart to confirmation.\n---\n\n## Steps\n\n1. Review the cart\n2. Pick a method\n3. Confirm\n",
    );
    write(CART_RULES, "---\ntitle: Cart rules\ndescription: Limits and discounts applied before payment.\n---\n\n## Limits\n\nA cart holds up to 20 line items. Each item allows a quantity of 1 to 10.\n\n## Discounts\n\nOne promo code per order.\n");
    write(ERROR_HANDLING, "---\ntitle: Error handling\n---\n\n## Declined cards\n\nShow the reason next to the card field.\n");
    write(RESEARCH, "---\ntitle: Checkout research\n---\n\n## Findings\n\nPeople abandon the cart when the total changes late.\n");
    write(COMPETITORS, "---\ntitle: Competitor flows\n---\n\n## Notes\n\nTwo of three competitors ask for the address first.\n");
    let mut manifest = Manifest::new();
    manifest.group_mut(Group::Overview).pages = vec![OVERVIEW.into()];
    manifest.group_mut(Group::Spec).pages = vec![PAYMENT_METHODS.into(), PAYMENT_FLOW.into(), CART_RULES.into(), ERROR_HANDLING.into()];
    manifest.group_mut(Group::Discovery).pages = vec![RESEARCH.into(), COMPETITORS.into()];
    let pin = |id: &str, name: &str, node: &str, group: Group, parent: Option<&str>| Pin {
        id: id.into(),
        file_id: file_id.into(),
        node_id: node.into(),
        name: name.into(),
        group,
        parent_screen: parent.map(str::to_string),
    };
    manifest.pins = vec![
        pin("cart", "Cart", "node-cart", Group::Screens, None),
        pin("payment", "Payment", "node-payment", Group::Screens, None),
        pin("confirmed", "Order confirmed", "node-confirmed", Group::Screens, None),
        pin("declined", "Card declined", "node-declined", Group::States, Some("payment")),
        pin("processing", "Processing", "node-processing", Group::States, Some("payment")),
    ];
    store.write_manifest(&manifest, Expect::Absent).unwrap();
}
