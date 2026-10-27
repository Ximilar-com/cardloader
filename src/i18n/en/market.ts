// market: the TCGplayer, Whatnot and Shopify exports (MarketExport) and the priced card table they share with
// eBay (PricedCards).

export default {
  conditionHint: {
    excellent: "lightly played",
    veryGood: "moderately played",
    poor: "heavily played",
  },
  priced: {
    allCards: "All cards",
    canBeListed: { one: "{count} card can be listed", other: "{count} cards can be listed" },
    canBeExported: { one: "{count} card can be exported", other: "{count} cards can be exported" },
    adjustPrices: "Adjust prices",
    priceUsd: "Price (USD)",
    percentOfMarket: "<field></field> % of the market price",
    percentLabel: "Percent of the market price",
    endIn99: "end in .99",
    atLeast: "at least $<field></field>",
    lowestPrice: "Lowest price",
    apply: "Apply to the chosen cards",
    choose: "Choose #{seq}",
    price: "Price",
    priceOf: "Price of #{seq}",
  },
  listingType: {
    auction: "Auction",
    auctionHint: "the price is the starting bid",
    buyItNow: "Buy it Now",
    buyItNowHint: "a fixed price",
  },
  shopifyFile: {
    products: "Products",
    productsHint: "new products with variants",
    inventory: "Inventory update",
    inventoryHint: "existing products",
  },
  shopifyGrouping: {
    byCard: "One product per card",
    byCardHint: "conditions are variants",
    perCard: "One product per scanned card",
  },
  export: {
    title: "Export for {name}",
    uploadingProgress: "Uploading pictures… {done}/{total}",
    preparingProgress: "Preparing… {done}/{total}",
    total: "{cards} · {total}",
    withoutPrice: { one: "{count} without a price", other: "{count} without a price" },
    stop: "Stop",
    exporting: "Exporting…",
    exportCsv: "Export CSV…",
    tcgplayerIntro:
      "A list of the ungraded trading cards in TCGplayer’s terms — product line, set, name, number, condition and your price — with each card’s TCGplayer product link. TCGplayer imports only files from its own Seller Portal export, so add the cards in Seller Portal from this list. Graded cards are sold there as Listings with Photos, one by one, and TCGplayer has no sports cards.",
    shopifyInventoryIntro:
      "Shopify’s inventory file (Products → Inventory → Import): sets each product’s stock at the location to the number of copies here. It finds the products this session’s product export made — same grouping — by their handles and conditions.",
    shopifyProductsIntro:
      "Shopify’s product file (Products → Import). Cards are found again by their handles, so tick “Overwrite products with matching handles” to update products imported before. Graded slabs are products of their own. In stores with several locations, set stock with the inventory update afterwards.",
    whatnotIntro:
      "Whatnot’s bulk listing file: import it in Seller Hub (Inventory → Import from CSV) for draft listings, or in a show (Add → Create Temporary Listing → Upload CSV). Graded slabs are listed as Graded with the grader and grade in the title and description. Prices are whole dollars, rounded up.",
    type: "Type",
    listingType: "Listing type",
    shippingProfile: "Shipping profile",
    file: "File",
    shopifyFile: "Shopify file",
    products: "Products",
    condition: "Condition of ungraded cards",
    conditionNote: "Cards whose condition Ximilar graded keep theirs.",
    location: "Location",
    locationPlaceholder: "e.g. Shop location",
    locationNote: "Exactly as named in Shopify (Settings → Locations).",
    uploadPictures: "Upload the pictures (ImgBB)",
    publishActive: "Publish (active) instead of drafts",
    offerable: "Buyers can make offers",
    noImgbbKey:
      "{name} downloads the pictures from links. Add an ImgBB API key in Settings to upload them; without one, add the photos in {name}.",
    loadingCards: "Loading cards…",
    noCards: "No identified cards in this session yet.",
    variantNote: "Copies of a card in the same condition become one variant, at the highest of their prices.",
  },
  result: {
    title: "Exported for {name}",
    showFile: "Show file",
    done: "Done",
    products: { one: "{count} product", other: "{count} products" },
    variants: { one: "{count} variant", other: "{count} variants" },
    productsVariants: "{products} ({variants})",
    saved: "{what} saved to <path></path>.",
    withoutPictures: {
      one: "{count} without pictures: add them in {name} before publishing.",
      other: "{count} without pictures: add them in {name} before publishing.",
    },
    leftOut: "Left out:",
  },
};
