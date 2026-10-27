// ebay: the table badge, the detail panel section and the "List on eBay" dialog (Ebay.tsx).

export default {
  status: {
    draft: "Draft",
    listed: "Listed",
    sold: "Sold",
    ended: "Ended",
  },
  badge: {
    draft: "eBay: draft",
    listed: "eBay: listed",
    sold: "eBay: sold",
    ended: "eBay: ended",
  },
  section: {
    notOnEbay: "Not on eBay",
    listing: "Listing {id}",
    draftNote: "An unpublished listing: publish it, or edit it on eBay.",
    publish: "Publish",
    endListing: "End listing",
    remove: "Remove from eBay",
  },
  list: {
    title: "List on eBay",
    connectFirst: "Connect your eBay account first.",
    policiesFirst: "Choose your shipping, payment and return policies and where cards ship from first.",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} without a price", other: "{count} without a price" },
    starting: "Starting…",
    createDrafts: { one: "Create {count} draft", other: "Create {count} drafts" },
    intro:
      "Each card becomes an unpublished listing (a draft) on eBay, with its pictures, at a fixed price. Check the drafts, then publish them from the session — or on eBay. Prices come from Ximilar where it has one.",
    introSandbox:
      "Each card becomes an unpublished listing (a draft) on eBay’s sandbox, with its pictures, at a fixed price. Check the drafts, then publish them from the session — or on eBay. Prices come from Ximilar where it has one.",
    condition: "Condition of ungraded cards",
    gradedNote: "Cards in a graded slab are listed with their grader and grade.",
    loadingCards: "Loading cards…",
    noCards: "No identified cards in this session yet.",
  },
  note: {
    sold: "Sold on eBay",
    live: "Live on eBay — a new price applies to the listing · {note}",
    draft: "A draft on eBay already — listing it again updates it · {note}",
  },
};
