(ns example.core)

; This comment should be removed
;; This comment should also be removed
(def greeting "Hello ; not a comment")

(defn add [a b]
  ;; TODO: add validation
  (+ a b))

;; noqa
(def x 42)
