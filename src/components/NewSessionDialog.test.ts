import { describe, expect, it } from "vitest";
import type { DeviceInfo, SessionOptions } from "../lib/api";
import { pairingAfterSource, scannerOf } from "./NewSessionDialog";

describe("pairingAfterSource", () => {
  const options = (o: { pairing?: "single" | "front_back"; bothSides?: boolean; duplex?: boolean }) =>
    ({
      pairing: o.pairing ?? "single",
      bothSides: o.bothSides ?? false,
      scan: { duplex: o.duplex ?? true },
    }) as SessionOptions;

  it("gives front + back for a duplex scanner and goes back when leaving it", () => {
    expect(pairingAfterSource(options({}), "folder", "scanner", "single")).toBe("front_back");
    const scanning = options({ pairing: "front_back" });
    expect(pairingAfterSource(scanning, "scanner", "folder", "single")).toBe("single");
    expect(pairingAfterSource(scanning, "scanner", "watch_folder", "single")).toBe("single");
    // The user's own choice (or a default of front + back) is what it goes back to.
    expect(pairingAfterSource(scanning, "scanner", "folder", "front_back")).toBe("front_back");
    // Reading the backs needs each back paired with its front.
    expect(pairingAfterSource(options({ pairing: "front_back", bothSides: true }), "scanner", "folder", "single")).toBe(
      "front_back",
    );
  });

  it("keeps it on other changes", () => {
    expect(pairingAfterSource(options({ pairing: "front_back" }), "folder", "watch_folder", "single")).toBe(
      "front_back",
    );
    expect(pairingAfterSource(options({ duplex: false }), "folder", "scanner", "front_back")).toBe("single");
    expect(pairingAfterSource(options({ duplex: false }), "scanner", "scanner", "front_back")).toBe("single");
  });
});

describe("scannerOf", () => {
  const devices: DeviceInfo[] = [
    { id: "dev-a", name: "Scanner A", backend: "sane" },
    { id: "dev-b", name: "Scanner B", backend: "sane" },
  ];

  it("keeps the default scanner whichever answer comes first", () => {
    // The settings first: the default, until the list is known.
    expect(scannerOf(null, "dev-b")).toBe("dev-b");
    // The list first: its first scanner, until the default is known.
    expect(scannerOf(devices, "")).toBe("dev-a");
    // Both: the default, as it is in the list.
    expect(scannerOf(devices, "dev-b")).toBe("dev-b");
  });

  it("takes the first scanner found when the wanted one isn't", () => {
    expect(scannerOf(devices, "unplugged")).toBe("dev-a");
    expect(scannerOf([], "unplugged")).toBe("unplugged");
  });
});
