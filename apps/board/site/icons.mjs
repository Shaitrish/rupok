// Platform glyphs from the Solar icon set (@iconify-json/solar, CC BY 4.0),
// shared by the /download/ page (site/pages/download.mjs) and src/download.ts.
// Plain JavaScript so both the Node page builder and Vite can import it.

const svg = (body) => `<svg viewBox="0 0 24 24" aria-hidden="true">${body}</svg>`;

export const DL_ICONS = {
  windows: svg("<g fill=\"none\" stroke=\"currentColor\" stroke-linecap=\"round\" stroke-width=\"1.5\"><path d=\"M8 17H16C18.8284 17 20.2426 17 21.1213 16.1213C22 15.2426 22 13.8284 22 11V10C22 6.22876 22 4.34315 20.8284 3.17157C19.6569 2 17.7712 2 14 2H10C6.22876 2 4.34315 2 3.17157 3.17157C2 4.34315 2 6.22876 2 10V11C2 13.8284 2 15.2426 2.87868 16.1213C3.75736 17 5.17157 17 8 17Z\"/><path d=\"M16 22H8\"/><path d=\"M12 17L12 22\"/><path d=\"M22 13H2\"/></g>"),
  mac: svg("<g fill=\"none\" stroke=\"currentColor\" stroke-linecap=\"round\" stroke-width=\"1.5\"><path d=\"M19.6471 15.5357H4.35294M19.6471 15.5357V8C19.6471 6.11438 19.6471 5.17157 19.0613 4.58579C18.4755 4 17.5327 4 15.6471 4H8.35294C6.46732 4 5.52451 4 4.93873 4.58579C4.35294 5.17157 4.35294 6.11438 4.35294 8V15.5357M19.6471 15.5357L21.3911 17.3358C21.4356 17.3818 21.4579 17.4048 21.4787 17.4276C21.7998 17.7802 21.9843 18.2358 21.999 18.7124C22 18.7433 22 18.7753 22 18.8393C22 18.9885 22 19.0631 21.996 19.1261C21.9325 20.1314 21.1314 20.9325 20.1261 20.996C20.0631 21 19.9885 21 19.8393 21H4.16068C4.01148 21 3.93688 21 3.87388 20.996C2.86865 20.9325 2.06749 20.1314 2.00398 19.1261C2 19.0631 2 18.9885 2 18.8393C2 18.7753 2 18.7433 2.00096 18.7124C2.01569 18.2358 2.20022 17.7802 2.52127 17.4276C2.54208 17.4048 2.56438 17.3818 2.60888 17.3358L4.35294 15.5357\"/><path d=\"M9.5 18.5H14.5\"/><path stroke-linejoin=\"round\" d=\"M12 6.75H12.0001\"/></g>"),
  linux: svg("<g fill=\"none\" stroke=\"currentColor\" stroke-linecap=\"round\" stroke-width=\"1.5\"><path d=\"M12 22C7.28595 22 4.92893 22 3.46447 20.5355C2 19.0711 2 16.714 2 12C2 7.28595 2 4.92893 3.46447 3.46447C4.92893 2 7.28595 2 12 2C16.714 2 19.0711 2 20.5355 3.46447C22 4.92893 22 7.28595 22 12C22 16.714 22 19.0711 20.5355 20.5355C19.0711 22 16.714 22 12 22Z\"/><path d=\"M17 15H14.5H12\"/><path d=\"M7 10L7.2344 10.1953C8.51608 11.2634 9.15693 11.7974 9.15693 12.5C9.15693 13.2026 8.51608 13.7366 7.2344 14.8047L7 15\"/></g>"),
  mobile: svg("<g fill=\"none\" stroke=\"currentColor\" stroke-linecap=\"round\" stroke-width=\"1.5\"><path d=\"M12 22C8.22876 22 6.34315 22 5.17157 20.8284C4 19.6569 4 17.7712 4 14V10C4 6.22876 4 4.34315 5.17157 3.17157C6.34315 2 8.22876 2 12 2C15.7712 2 17.6569 2 18.8284 3.17157C20 4.34315 20 6.22876 20 10V14C20 17.7712 20 19.6569 18.8284 20.8284C17.6569 22 15.7712 22 12 22Z\"/><path d=\"M15 19H9\"/></g>"),
  download: svg("<g fill=\"none\" stroke=\"currentColor\" stroke-linecap=\"round\" stroke-linejoin=\"round\" stroke-width=\"1.5\"><path d=\"M3 15C3 17.8284 3 19.2426 3.87868 20.1213C4.75736 21 6.17157 21 9 21H15C17.8284 21 19.2426 21 20.1213 20.1213C21 19.2426 21 17.8284 21 15\"/><path d=\"M12 3V16M8 11.625L12 16L16 11.625\"/></g>"),
};

/** Fallback when the GitHub API is unavailable or rate-limited. */
export const RELEASES_URL = "https://github.com/Shaitrish/rupok/releases/latest";
export const RELEASES_API = "https://api.github.com/repos/Shaitrish/rupok/releases/latest";
/** Asset kinds, picked from a release by file-name suffix. */
export const ASSET_SUFFIX = { exe: "x64-setup.exe", msi: ".msi", deb: "amd64.deb", appimage: ".AppImage", dmg: ".dmg" };
