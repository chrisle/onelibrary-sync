import { expect, it } from "vitest";
import { formatFileSystem } from "./format";

it("writes file system names in capitals", () => {
  expect(["hfs", "exFAT", "FAT32", "apfs"].map(formatFileSystem)).toEqual(["HFS", "EXFAT", "FAT32", "APFS"]);
});
