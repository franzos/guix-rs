;;; installed_ops.scm — read-side introspection over the user profile.
;;;
;;; Exposes a single helper to Rust: walks the manifest at PROFILE-PATH
;;; and emits `(name version source-file (channel-name ...))` tuples for
;;; every entry. The Rust side groups packages by channel name.
;;;
;;; Provenance is computed by `(guix describe) package-channels` — the
;;; same routine `guix describe` and `guix package --list-profiles` use.
;;; It resolves the package's `(package-location)` source-file against
;;; the pull-profile manifest entries' store-prefixes, which carry the
;;; per-channel `source` property written by `channel-instances->manifest`.
;;;
;;; `package-channels` returns at least the `guix` channel for any package
;;; coming from `(gnu packages …)`, so the typical result is a single-
;;; element list. An empty list means attribution failed — Rust buckets
;;; that case under `(unknown)`.

;; Response shape mirrors channel_ops: `(ok (<entry> ...))` on success,
;; `(error <message>)` on failure. Rust surfaces the error rather than
;; rendering an empty profile.
(define (libguix-rs:installed-with-locations profile-path)
  (catch
   #t
   (lambda ()
     (let* ((m (profile-manifest profile-path))
            (entries (manifest-entries m)))
       (list 'ok
             (map (lambda (e)
                    (let* ((name    (manifest-entry-name e))
                           (version (manifest-entry-version e))
                           (pkg     (false-if-exception
                                     (specification->package name)))
                           (source  (libguix-rs:package-source-file pkg))
                           (chans   (libguix-rs:package-channel-names pkg)))
                      (list name version source chans)))
                  entries))))
   (lambda (key . args)
     (list 'error (format #f "~a: ~a" key args)))))

;; The package's source-file (the `.scm` defining it), as recorded in
;; `(package-location)`. Returns "" if unavailable — Rust tolerates the
;; empty string and treats it as a missing field.
(define (libguix-rs:package-source-file pkg)
  (let* ((loc (and pkg (false-if-exception (package-location pkg))))
         (file (and loc (false-if-exception (location-file loc)))))
    (if (string? file) file "")))

;; Wraps `(guix describe) package-channels`. Returns a list of channel
;; names as strings. Empty list on failure or when attribution couldn't
;; be determined — Rust buckets that case under `(unknown)`.
(define (libguix-rs:package-channel-names pkg)
  (if (not pkg)
      '()
      (let ((channels (false-if-exception (package-channels pkg))))
        (if (list? channels)
            (map (lambda (ch)
                   (symbol->string (channel-name ch)))
                 channels)
            '()))))
