;;; org-features.el --- count Org constructs per file with org-element  -*- lexical-binding: t; -*-
;; Usage: emacs --batch -l org-features.el FILE... > out.jsonl
;; Prints one JSON object per file: {"file": ..., "counts": {"<feature>": n, ...}}.
;; Features are "type" or "type:detail", e.g. "src-block", "src-block:rust",
;; "keyword:TITLE", "special-block:theorem", "link:https".

(require 'org)
(require 'org-element)
(require 'json)

(defun of--bump (table key)
  (puthash key (1+ (gethash key table 0)) table))

(defun of--latex-fragment-kind (value)
  (cond ((string-prefix-p "\\(" value) "inline-paren")
        ((string-prefix-p "\\[" value) "display-bracket")
        ((string-prefix-p "$$" value) "display-dollars")
        ((string-prefix-p "$" value) "inline-dollar")
        (t "command")))

(defun of--latex-env-name (value)
  (if (string-match "\\\\begin{\\([^}]+\\)}" value) (downcase (match-string 1 value)) "?"))

(defun of--count-file (file)
  (let ((counts (make-hash-table :test 'equal))
        (org-inhibit-startup t)
        (enable-local-variables nil))
    (with-temp-buffer
      (insert-file-contents file)
      (let ((org-mode-hook nil)) (org-mode))
      (let ((tree (org-element-parse-buffer)))
        (org-element-map tree t
          (lambda (el)
            (let ((type (org-element-type el)))
              (unless (memq type '(org-data section paragraph plain-text))
                (of--bump counts (symbol-name type)))
              (pcase type
                ('headline
                 (when (org-element-property :todo-keyword el)
                   (of--bump counts (concat "headline:todo:" (substring-no-properties (org-element-property :todo-keyword el)))))
                 (when (org-element-property :priority el) (of--bump counts "headline:priority"))
                 (when (org-element-property :commentedp el) (of--bump counts "headline:COMMENT"))
                 (when (org-element-property :archivedp el) (of--bump counts "headline:ARCHIVE"))
                 (dolist (tag (org-element-property :tags el))
                   (let ((tg (substring-no-properties tag)))
                     (cond ((member tg '("noexport" "export" "ignore")) (of--bump counts (concat "headline:tag:" tg)))
                           (t (of--bump counts "headline:tag:other")))))
                 (of--bump counts (format "headline:level:%d" (min 6 (org-element-property :level el)))))
                ('keyword (of--bump counts (concat "keyword:" (upcase (org-element-property :key el)))))
                ('affiliated-keyword nil)
                ('special-block (of--bump counts (concat "special-block:" (downcase (org-element-property :type el)))))
                ('src-block (of--bump counts (concat "src-block:" (downcase (or (org-element-property :language el) "none"))))
                            (when (org-element-property :parameters el) (of--bump counts "src-block:has-header-args")))
                ('inline-src-block (of--bump counts (concat "inline-src-block:" (downcase (or (org-element-property :language el) "none")))))
                ('export-block (of--bump counts (concat "export-block:" (downcase (org-element-property :type el)))))
                ('export-snippet (of--bump counts (concat "export-snippet:" (downcase (org-element-property :back-end el)))))
                ('latex-environment (of--bump counts (concat "latex-environment:" (of--latex-env-name (org-element-property :value el)))))
                ('latex-fragment (of--bump counts (concat "latex-fragment:" (of--latex-fragment-kind (org-element-property :value el)))))
                ('link (of--bump counts (concat "link:" (org-element-property :type el)))
                       (when (org-element-contents el) (of--bump counts "link:has-description")))
                ('plain-list (of--bump counts (concat "plain-list:" (symbol-name (org-element-property :type el)))))
                ('item (when (org-element-property :checkbox el) (of--bump counts "item:checkbox"))
                       (when (org-element-property :counter el) (of--bump counts "item:counter")))
                ('table (of--bump counts (concat "table:" (symbol-name (org-element-property :type el))))
                        (when (org-element-property :tblfm el) (of--bump counts "table:tblfm")))
                ('drawer (of--bump counts (concat "drawer:" (upcase (org-element-property :drawer-name el)))))
                ('node-property (of--bump counts (concat "node-property:" (upcase (org-element-property :key el)))))
                ('macro (of--bump counts (concat "macro:" (downcase (org-element-property :key el)))))
                ('timestamp (of--bump counts (concat "timestamp:" (symbol-name (org-element-property :type el)))))
                ('dynamic-block (of--bump counts (concat "dynamic-block:" (org-element-property :block-name el))))
                ('citation (of--bump counts (concat "citation:style:" (or (org-element-property :style el) "default"))))
                ('footnote-reference (of--bump counts (concat "footnote-reference:" (symbol-name (or (org-element-property :type el) 'standard))))))
              ;; Affiliated keywords (#+NAME, #+CAPTION, #+ATTR_*, #+RESULTS) live as properties on elements.
              (when (memq type org-element-all-elements)
                (when (org-element-property :name el) (of--bump counts "affiliated:NAME"))
                (when (org-element-property :caption el) (of--bump counts "affiliated:CAPTION"))
                (when (org-element-property :results el) (of--bump counts "affiliated:RESULTS"))
                (let ((plist (cadr el)))
                  (while plist
                    (let ((k (symbol-name (car plist))))
                      (when (string-prefix-p ":attr_" k)
                        (of--bump counts (concat "affiliated:" (upcase (substring k 1))))))
                    (setq plist (cddr plist)))))))
          nil nil nil t)))
    counts))

(defun of--hash-to-alist (h)
  (let (al) (maphash (lambda (k v) (push (cons k v) al)) h)
       (sort al (lambda (a b) (string< (car a) (car b))))))

(dolist (file command-line-args-left)
  (condition-case err
      (princ (concat (json-encode `((file . ,file) (counts . ,(of--hash-to-alist (of--count-file file))))) "\n"))
    (error (princ (concat (json-encode `((file . ,file) (error . ,(format "%S" err)))) "\n")))))
(setq command-line-args-left nil)
