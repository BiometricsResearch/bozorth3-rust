use crate::consts::MAX_NUMBER_OF_MINUTIAE;
use crate::types::Endpoint;

pub(crate) struct EndpointAssociations {
    probe_by_gallery: [u8; MAX_NUMBER_OF_MINUTIAE],
    gallery_by_probe: [u8; MAX_NUMBER_OF_MINUTIAE],
}

impl EndpointAssociations {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            probe_by_gallery: [0; MAX_NUMBER_OF_MINUTIAE],
            gallery_by_probe: [0; MAX_NUMBER_OF_MINUTIAE],
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.probe_by_gallery.iter_mut().for_each(|it| *it = 0);
        self.gallery_by_probe.iter_mut().for_each(|it| *it = 0);
    }

    #[inline]
    pub(crate) fn associate(&mut self, probe_endpoint: Endpoint, gallery_endpoint: Endpoint) {
        self.probe_by_gallery[gallery_endpoint.as_usize()] = probe_endpoint.as_usize() as u8 + 1;
        self.gallery_by_probe[probe_endpoint.as_usize()] = gallery_endpoint.as_usize() as u8 + 1;
    }

    #[inline]
    pub(crate) fn clear_by_probe(&mut self, probe_endpoint: Endpoint) {
        let value = self.gallery_by_probe[probe_endpoint.as_usize()];
        if value != 0 {
            self.probe_by_gallery[(value - 1) as usize] = 0;
            self.gallery_by_probe[probe_endpoint.as_usize()] = 0;
        }
    }

    #[inline]
    pub(crate) fn get_associated_by_gallery(&self, gallery_endpoint: Endpoint) -> Option<Endpoint> {
        let endpoint = self.probe_by_gallery[gallery_endpoint.as_usize()];
        if endpoint != 0 {
            Some((endpoint - 1).into())
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn get_associated_by_probe(&self, probe_endpoint: Endpoint) -> Option<Endpoint> {
        let endpoint = self.gallery_by_probe[probe_endpoint.as_usize()];
        if endpoint != 0 {
            Some((endpoint - 1).into())
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn get_status(
        &self,
        probe_endpoint: Endpoint,
        gallery_endpoint: Endpoint,
    ) -> EndpointRelation {
        let associated_gallery = self.gallery_by_probe[probe_endpoint.as_usize()];
        let associated_probe = self.probe_by_gallery[gallery_endpoint.as_usize()];
        if associated_gallery == 0 && associated_probe == 0 {
            return EndpointRelation::Unassociated;
        }

        if associated_gallery == gallery_endpoint.as_usize() as u8 + 1
            && associated_probe == probe_endpoint.as_usize() as u8 + 1
        {
            return EndpointRelation::MutuallyAssociated;
        }

        EndpointRelation::AssociatedToOther
    }
}

pub(crate) enum EndpointRelation {
    Unassociated,
    MutuallyAssociated,
    AssociatedToOther,
}
