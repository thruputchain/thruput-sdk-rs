X23=~/gh/all/sdk_rs/hogd/x23
XDRC="$X23/x23c/x23c"
protrs="/home/dbg/gh/all/sdk_rs/protocol-rs"

function mkx23 {
  (cd "$X23" && make)
}

function genrs {
  (cd $protrs && XDRC="$XDRC" scripts/generate.sh)
  (cd $protrs/groundhog_xdr/src && git st)
}
