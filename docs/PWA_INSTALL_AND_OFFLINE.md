# LIFEBOAT PWA installation and offline model

The repository root is published at the GitHub Pages project URL `https://tajoor621.github.io/LIFEBOAT/`.

## Installability

- `manifest.webmanifest` defines the LIFEBOAT identity, project-site scope, start URL, display mode, theme and SVG app icon.
- `index.html` links the manifest, theme metadata and service worker.
- `pwa.js` exposes the browser install prompt where supported.
- The Pages site must be deployed from GitHub Actions (the workflow in `.github/workflows/pages.yml`).

## Offline and privacy boundary

The service worker caches only the static PWA shell (HTML, CSS, JavaScript, manifest and public icon). It does not cache API requests, navigations to external origins, contacts, VCF exports, backup payloads, credentials or encryption keys. There is no background sync or push subscription. Offline availability means only that this static information shell can open; it does not mean backup, restore or cloud synchronization is available offline.

The service worker uses a versioned cache and removes obsolete LIFEBOAT shell caches during activation. A new deployment takes effect when the browser activates the updated worker. Users can close old tabs or reload after the update.

## Android identity and APK installation

The native Android application ID is `com.tajoor621.lifeboat`. It is separate from the PWA manifest ID and from the GitHub repository name. The APK workflow validates package metadata, alignment and signature, and installs/launches the APK on an Android emulator before artifact publication. The current debug APK remains a test build. A stable production signing key must be stored as protected GitHub Actions secrets before release APKs can support reliable upgrades across CI runs; never commit a keystore or passwords to the repository.
