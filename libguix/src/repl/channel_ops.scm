;;; channel_ops.scm — channels writer (called via the REPL actor).
;;;
;;; The Rust side serialises a `ChannelOp` to an op-sexp and evaluates
;;;   (libguix-rs:apply-channel-op <source-string> '<op-sexp>)
;;; in the persistent `libguix-rs` namespace (primed once at actor
;;; bootstrap).
;;;
;;; Response shape:
;;;   (ok "<new-source>")
;;;   (error <symbol> "<msg>" <line-or-#f> <col-or-#f>)
;;;
;;; Phase 1b: add-channel + remove-channel-by-name, comment-preserving
;;; round-trips via `(guix read-print) read-with-comments` /
;;; `pretty-print-with-comments`.

(define (libguix-rs:apply-channel-op source op-sexp)
  (catch
   #t
   (lambda ()
     ;; Parse with `read-with-comments` so the AST carries the <blank>
     ;; / <comment> records that `pretty-print-with-comments` needs to
     ;; round-trip inline comments and vertical whitespace.
     (call-with-input-string
      source
      (lambda (port)
        (let loop ((forms '()))
          (let ((form (read-with-comments port)))
            (cond
             ((eof-object? form)
              ;; Walk every collected form, find the first whose head is
              ;; list / cons / cons*. Apply op to its element list,
              ;; replace in-place, re-emit all forms.
              (let* ((all-forms (reverse forms))
                     (idx (libguix-rs:find-channels-index all-forms)))
                (if (not idx)
                    (list 'error 'parse-error
                          "no `list` / `cons*` / `cons` form found"
                          #f #f)
                    (let* ((channels-form (list-ref all-forms idx))
                           (new-form (libguix-rs:apply-op-to-form
                                      channels-form op-sexp)))
                      (cond
                       ((and (pair? new-form) (eq? (car new-form) 'error))
                        new-form)
                       (else
                        (let ((updated
                               (libguix-rs:replace-at all-forms idx new-form)))
                          (list 'ok
                                (libguix-rs:emit-forms updated)))))))))
             (else
              (loop (cons form forms)))))))))
   (lambda (key . args)
     (list 'error 'eval-error
           (format #f "~a: ~a" key args)
           #f #f))))

;; Returns the 0-based index of the first non-blank form whose head
;; (sans comment annotations) is `list`, `cons*` or `cons`, or #f.
(define (libguix-rs:find-channels-index forms)
  (let loop ((fs forms) (i 0))
    (cond
     ((null? fs) #f)
     ((let ((h (libguix-rs:form-head (car fs))))
        (and h (memq h '(list cons cons*))))
      i)
     (else (loop (cdr fs) (+ i 1))))))

;; Returns the head symbol of `form` (the result of `read-with-comments`),
;; or #f if `form` is a blank/comment or doesn't have a symbol head.
;; `read-with-comments` returns <blank>/<comment> records at top level
;; (they aren't wrappers — they sit in the list alongside real forms),
;; so we just need to skip them rather than unwrap them.
(define (libguix-rs:form-head form)
  (cond
   ((blank? form) #f)
   ((and (pair? form) (symbol? (car form))) (car form))
   (else #f)))

;; Replace the element at index `idx` with `new`, keeping order.
(define (libguix-rs:replace-at lst idx new)
  (let loop ((ls lst) (i 0) (acc '()))
    (cond
     ((null? ls) (reverse acc))
     ((= i idx) (loop (cdr ls) (+ i 1) (cons new acc)))
     (else (loop (cdr ls) (+ i 1) (cons (car ls) acc))))))

;; Dispatch on op-sexp head.
(define (libguix-rs:apply-op-to-form channels-form op-sexp)
  (cond
   ((not (pair? op-sexp))
    (list 'error 'unsupported-op "op must be a list" #f #f))
   ((eq? (car op-sexp) 'add-channel)
    (libguix-rs:op-add-channel channels-form (cadr op-sexp)))
   ((eq? (car op-sexp) 'remove-channel-by-name)
    (libguix-rs:op-remove-channel-by-name channels-form (cadr op-sexp)))
   (else
    (list 'error 'unsupported-op
          (format #f "unknown op ~a" (car op-sexp)) #f #f))))

;; Append (Explicit) or insert-before-tail (WithDefaults) the new
;; channel into the channels form. The channel sexp must include an
;; introduction (Rust pre-flight enforces this).
(define (libguix-rs:op-add-channel channels-form new-channel)
  (let* ((head (car channels-form))
         (elements (cdr channels-form)))
    (cond
     ((eq? head 'list)
      ;; Explicit: append after all existing elements (inline comments
      ;; attached to earlier channels stay attached).
      (cons 'list (append elements (list new-channel))))
     ((or (eq? head 'cons*) (eq? head 'cons))
      ;; WithDefaults: tail must be `%default-channels`; insert before it.
      ;; The tail may be preceded by trailing blanks; we need to insert
      ;; the new channel before any trailing blanks AND before the tail
      ;; symbol itself.
      (let* ((split (libguix-rs:split-trailing-tail elements))
             (head-elts (car split))
             (trailing-blanks (cadr split))
             (tail (caddr split)))
        (cond
         ((not (eq? tail '%default-channels))
          (list 'error 'parse-error
                "cons*/cons form does not end in `%default-channels`"
                #f #f))
         (else
          ;; Promote `cons` to `cons*` if necessary — `cons` only takes
          ;; two args, so adding a custom channel requires `cons*`.
          (cons 'cons*
                (append head-elts
                        (list new-channel)
                        trailing-blanks
                        (list '%default-channels)))))))
     (else
      (list 'error 'parse-error
            (format #f "unexpected channels head ~a" head) #f #f)))))

;; Drop the first channel element whose inner `(channel …)` matches
;; `name-sym`. The match drops the whole element (wrapper included) plus
;; any immediately preceding <blank>/<comment> records (so inline
;; alternate-URL comments attached to the channel go with it).
;;
;; If the matched element is the only channel in a `cons` form, the
;; result becomes the bare `%default-channels` symbol — Guile evaluates
;; that as the default channels list. Same for `cons*` collapsing down
;; to just `%default-channels`.
(define (libguix-rs:op-remove-channel-by-name channels-form name-sym)
  (let* ((head (car channels-form))
         (elements (cdr channels-form))
         (matched? #f)
         ;; Walk elements left-to-right, accumulating into `acc` (reversed).
         ;; On match, drop the element AND any immediately preceding
         ;; <blank>/<comment> records (those are the inline alternate-URL
         ;; comments attached to this channel — at the head of `acc`).
         (kept
          (let loop ((es elements) (acc '()))
            (cond
             ((null? es) acc)
             ((and (not matched?)
                   (libguix-rs:element-channel-name-is? (car es) name-sym))
              (set! matched? #t)
              (loop (cdr es) (libguix-rs:drop-leading-blanks acc)))
             (else
              (loop (cdr es) (cons (car es) acc)))))))
    (cond
     ((not matched?)
      (list 'error 'not-found
            (symbol->string name-sym) #f #f))
     (else
      (let* ((new-elements (reverse kept))
             ;; Count real (non-blank) channel-like elements remaining.
             (real-count (libguix-rs:count-non-blanks new-elements)))
        (cond
         ((and (or (eq? head 'cons) (eq? head 'cons*))
               ;; If only `%default-channels` (+ optional blanks) remain,
               ;; collapse to the bare symbol.
               (libguix-rs:only-default-channels? new-elements))
          '%default-channels)
         ((eq? head 'cons)
          ;; `cons` requires exactly two args. If a non-tail element was
          ;; removed and we now have just one non-blank custom channel
          ;; left plus `%default-channels`, keep `cons`. Otherwise we
          ;; need to switch shape — but pre-flight only allows removing
          ;; names that ARE in the custom list, so for `cons` the only
          ;; reachable state here is: matched the sole custom channel.
          ;; That case is handled above (collapse). Defensive fallback:
          ;; promote to cons*.
          (cons 'cons* new-elements))
         (else
          (cons head new-elements))))))))

;; Returns #t if `elt` is a channel (possibly wrapped) whose inner
;; `(channel (name 'NAME) …)` has `(eq? NAME name-sym)`.
(define (libguix-rs:element-channel-name-is? elt name-sym)
  (let ((ch (libguix-rs:find-inner-channel elt)))
    (and ch (eq? (libguix-rs:channel-name ch) name-sym))))

;; Returns the inner `(channel …)` form found inside `elt`, walking
;; through up to two levels of wrappers (e.g.
;; `(channel-with-substitutes-available (channel …) "url")`). Skips
;; <blank> records. Returns #f if no `(channel …)` found.
(define (libguix-rs:find-inner-channel elt)
  (cond
   ((blank? elt) #f)
   ((not (pair? elt)) #f)
   ((not (symbol? (car elt))) #f)
   ((eq? (car elt) 'channel) elt)
   (else
    ;; Wrapper: look for the first child that is itself a `(channel …)`
    ;; form (skipping blanks and non-channel children).
    (let loop ((children (cdr elt)))
      (cond
       ((null? children) #f)
       ((blank? (car children)) (loop (cdr children)))
       ((and (pair? (car children))
             (symbol? (caar children))
             (eq? (caar children) 'channel))
        (car children))
       (else (loop (cdr children))))))))

;; Extracts the value of the `(name 'NAME)` field from a `(channel …)`
;; form. Returns the symbol, or #f if absent.
(define (libguix-rs:channel-name ch)
  (let loop ((fields (cdr ch)))
    (cond
     ((null? fields) #f)
     ((blank? (car fields)) (loop (cdr fields)))
     ((and (pair? (car fields))
           (eq? (car (car fields)) 'name)
           (pair? (cdr (car fields))))
      ;; `(name 'foo)` reads as `(name (quote foo))`.
      (let ((v (cadr (car fields))))
        (cond
         ((and (pair? v) (eq? (car v) 'quote)) (cadr v))
         ((symbol? v) v)
         (else #f))))
     (else (loop (cdr fields))))))

;; Drops leading <blank> records from a list. Used to strip
;; comments/blanks attached to the channel we just removed.
(define (libguix-rs:drop-leading-blanks lst)
  (cond
   ((null? lst) '())
   ((blank? (car lst)) (libguix-rs:drop-leading-blanks (cdr lst)))
   (else lst)))

;; Returns the count of non-<blank> elements in `lst`.
(define (libguix-rs:count-non-blanks lst)
  (let loop ((ls lst) (n 0))
    (cond
     ((null? ls) n)
     ((blank? (car ls)) (loop (cdr ls) n))
     (else (loop (cdr ls) (+ n 1))))))

;; Returns #t if `lst` contains exactly one non-blank element and it is
;; the symbol `%default-channels`.
(define (libguix-rs:only-default-channels? lst)
  (let loop ((ls lst) (seen-default? #f) (others? #f))
    (cond
     ((null? ls) (and seen-default? (not others?)))
     ((blank? (car ls)) (loop (cdr ls) seen-default? others?))
     ((eq? (car ls) '%default-channels)
      (loop (cdr ls) #t others?))
     (else (loop (cdr ls) seen-default? #t)))))

;; Splits a `cons` / `cons*` element list into (head-elts, trailing-blanks,
;; tail). The tail is the last non-blank element; trailing-blanks are any
;; <blank> records sitting between the tail and the last real channel.
(define (libguix-rs:split-trailing-tail elements)
  (let* ((rev (reverse elements))
         ;; The first non-blank in `rev` is the tail.
         (tail-and-rest
          (let loop ((rs rev) (blanks '()))
            (cond
             ((null? rs) (list #f '() '()))
             ((blank? (car rs)) (loop (cdr rs) (cons (car rs) blanks)))
             (else (list (car rs) blanks (cdr rs))))))
         (tail (car tail-and-rest))
         (trailing-blanks (cadr tail-and-rest))
         ;; The rest in original order, minus the tail.
         (head-elts (reverse (caddr tail-and-rest))))
    (list head-elts trailing-blanks tail)))

;; Re-emit forms via `pretty-print-with-comments` (from `(guix
;; read-print)`). One blank line between forms preserves the typical
;; preamble + channels-form layout.
(define (libguix-rs:emit-forms forms)
  (call-with-output-string
    (lambda (port)
      (let loop ((fs forms) (first? #t))
        (cond
         ((null? fs) #t)
         ((blank? (car fs))
          ;; Top-level blank/comment — emit it via pretty-print-with-comments
          ;; too; it handles <blank> records correctly.
          (pretty-print-with-comments port (car fs))
          (loop (cdr fs) #f))
         (else
          (unless first? (newline port))
          (pretty-print-with-comments port (car fs))
          (newline port)
          (loop (cdr fs) #f)))))))
