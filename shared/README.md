<div align="center">

  <h1><code>Devhub Shared</code></h1>

  <p>
    <strong>Proposal and Rfp structs for both neardevhub-contract and neardevhub-cache-api</strong>
  </p>

   <h3>
      <a href="https://newsletter.neardevhub.org/">Newsletter</a>
      <span> | </span>
      <a href="https://github.com/near/near-sdk-rs#pre-requisites">Pre-requisites</a>
      <span> | </span>
      <a href="https://github.com/near/near-sdk-rs#writing-rust-contract">Writing Rust Contract</a>
      <span> | </span>
      <a href="https://github.com/near/near-sdk-rs#building-rust-contract">Building Rust Contract</a>
      <span> | </span>
      <a href="https://devhub.near.page/devhub.near/widget/app?page=blogv2">Blog</a>
      <span> | </span>
      <a href="https://devhub.near.page/devhub.near/widget/app?page=about">About</a>
    </h3>
</div>


# Example 

```rust
use devhub_shared::proposal::timeline::{
    TimelineStatus, TimelineStatusV1, VersionedTimelineStatus,
};

use devhub_shared::rfp::timeline::TimelineStatus as RFPTimelineStatus;

use devhub_shared::proposal::{
    Proposal, ProposalBodyV2, ProposalId, ProposalSnapshot, VersionedProposalBody,
};
```

## License
This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
