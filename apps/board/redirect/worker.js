// Permanent redirect of every path on alternate hostnames to the canonical site.
export default {
  fetch(request) {
    const url = new URL(request.url);
    return Response.redirect(`https://rupok.app${url.pathname}${url.search}`, 301);
  },
};
