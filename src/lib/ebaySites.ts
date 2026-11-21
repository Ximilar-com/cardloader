/** The eBay sites card prices can be searched on, as `browse::SEARCH_MARKETS` lists them (keep the two the same):
 * the marketplace id, the site and the currency its listings are priced in. */
export const EBAY_SITES = [
  { id: "EBAY_US", site: "ebay.com", currency: "USD" },
  { id: "EBAY_GB", site: "ebay.co.uk", currency: "GBP" },
  { id: "EBAY_DE", site: "ebay.de", currency: "EUR" },
  { id: "EBAY_FR", site: "ebay.fr", currency: "EUR" },
  { id: "EBAY_IT", site: "ebay.it", currency: "EUR" },
  { id: "EBAY_ES", site: "ebay.es", currency: "EUR" },
  { id: "EBAY_NL", site: "ebay.nl", currency: "EUR" },
  { id: "EBAY_BE", site: "ebay.be", currency: "EUR" },
  { id: "EBAY_AT", site: "ebay.at", currency: "EUR" },
  { id: "EBAY_IE", site: "ebay.ie", currency: "EUR" },
  { id: "EBAY_CH", site: "ebay.ch", currency: "CHF" },
  { id: "EBAY_PL", site: "ebay.pl", currency: "PLN" },
  { id: "EBAY_CA", site: "ebay.ca", currency: "CAD" },
  { id: "EBAY_AU", site: "ebay.com.au", currency: "AUD" },
  { id: "EBAY_HK", site: "ebay.com.hk", currency: "HKD" },
  { id: "EBAY_SG", site: "ebay.com.sg", currency: "SGD" },
] as const;

/** The sites as Select options: the site, with its currency as the hint. */
export function ebaySiteOptions(): { value: string; label: string; hint: string }[] {
  return EBAY_SITES.map((s) => ({ value: s.id, label: s.site, hint: s.currency }));
}

/** A site's domain ("ebay.de"), for an id the app may not list (an older one): the id itself. */
export function ebaySite(id: string | null | undefined): string {
  return EBAY_SITES.find((s) => s.id === id)?.site ?? id ?? "ebay.com";
}
