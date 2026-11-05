```mermaid
flowchart TD
    A[Cart] --> B{Paid?}
    B -->|yes| C[Receipt]
    B -->|no| A
```
