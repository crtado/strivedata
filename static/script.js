window.addEventListener("load", () => {
  let search = document.getElementById("search")
  search.addEventListener("keypress", event => {
    if (event.key === 'Enter') {
      let tag = search.value.trim();
      console.log(tag);
      window.location.href = '/search/' + encodeURIComponent(tag);
    }
  });
});